// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 该模块定义了oxcache的宏实现，提供缓存注解功能。

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Expr, ItemFn, Lit, Meta, Token, parse::Parser, punctuated::Punctuated, spanned::Spanned,
};

/// 为函数启用缓存装饰。
///
/// # 缓存键
///
/// 默认键为 `service:fn_name:<实参摘要>`：实参以 `Debug` 串参与 sha256
/// 摘要，**明文实参不进入键**（实参常含口令 / 会话令牌，明文进入持久化
/// 键空间即等于凭据落盘）。`skip(param)` 的参数不参与摘要。
///
/// 显式 `key = "..."` 模板时按键模板生成：占位符可引用函数参数、
/// `self` 或 `self.<字段路径>`（如 `key = "user_{id}"`、
/// `key = "user_{self.tenant_id}"`），占位符值按段分隔符转义后入键。
///
/// # 作用域（租户 / 用户维度）
///
/// 方法（带 `self` 接收者）的默认键不含任何归属维度，不同租户会共享
/// 同一条缓存条目。因此带 `self` 的函数**必须**给出作用域：
///
/// - `scope = <expr>`：任意表达式，在函数体内求值并作为键的一段
///   （方法可用 `scope = self.tenant_id`）；或
/// - `key = "..."`：显式键模板，且模板必须引用归属维度（占位符指向
///   参数或 `self` 字段路径）。
///
/// 两者都缺失、或方法上的键模板是常量时，编译期报错，不做静默放行。
///
/// # 参数
///
/// - `service = "..."`：缓存服务名（默认 `"default"`）；
/// - `ttl = N`：缓存 TTL 秒数；
/// - `key = "..."`：显式键模板（占位符引用参数 / `self` 字段路径）；
/// - `key_prefix = "..."`：键前缀（静态字面量，不承载作用域）；
/// - `scope = <expr>`：作用域表达式（键的归属维度）；
/// - `condition = path`：命中前的前置谓词；
/// - `skip(param, ...)`：不参与键摘要的参数；
/// - `sync`：生成同步版本（不可与 `async fn` 同用）；
/// - `single_flight`：并发未命中去重；
/// - `strict`：服务未注册时 panic；
/// - `cache_none`：`Ok(None)` 也写回缓存；
/// - `skip_cache_write`：只读缓存，不写回。
#[proc_macro_attribute]
pub fn cached(args: TokenStream, item: TokenStream) -> TokenStream {
    cached_impl(args.into(), item.into()).into()
}

/// 显式键模板的解析结果
///
/// 占位符按「参数名 / `self` / `self.<字段路径>`」解析，值在运行期经
/// `oxcache::macro_support::escape_key_segment` 转义后按模板产出。
struct KeyTemplate {
    /// 重写后的模板：占位符统一改写为 `__oxcache_key_arg_N`
    pattern: String,
    /// 占位符绑定：(绑定名, 取值表达式)
    bindings: Vec<(syn::Ident, proc_macro2::TokenStream)>,
    /// 模板是否含占位符（常量模板不承载归属维度）
    has_placeholder: bool,
}

/// 解析 `key = "..."` 模板中的占位符
///
/// 未闭合的 `{` 按字面保留；`{{` / `}}` 为花括号转义，原样透传。
fn parse_key_template(pattern: &str, arg_names: &[&syn::Ident]) -> syn::Result<KeyTemplate> {
    let span = proc_macro2::Span::call_site();
    let mut out = String::with_capacity(pattern.len());
    let mut bindings: Vec<(syn::Ident, proc_macro2::TokenStream)> = Vec::new();
    let mut has_placeholder = false;
    let mut rest = pattern;
    while let Some(open) = rest.find('{') {
        if rest[open..].starts_with("{{") {
            out.push_str(&rest[..open]);
            out.push('{');
            rest = &rest[open + 2..];
            continue;
        }
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            // 未闭合的 `{`：按字面保留
            out.push_str(&rest[open..]);
            return Ok(KeyTemplate {
                pattern: out,
                bindings,
                has_placeholder,
            });
        };
        let inner = &after[..close];
        let (name, spec) = match inner.find(':') {
            Some(idx) => (&inner[..idx], Some(&inner[idx + 1..])),
            None => (inner, None),
        };
        let expr = resolve_key_placeholder(name, arg_names, span)?;
        let ident = syn::Ident::new(&format!("__oxcache_key_arg_{}", bindings.len()), span);
        let value = match spec {
            // `?` 是 Debug 简写：取值用 Debug 形态，转义后按普通占位符产出
            Some("?") => quote! { ::std::format!("{:?}", #expr) },
            _ => quote! { ::std::format!("{}", #expr) },
        };
        bindings.push((ident.clone(), value));
        has_placeholder = true;
        out.push('{');
        out.push_str(&ident.to_string());
        if let Some(spec) = spec
            && spec != "?"
        {
            out.push(':');
            out.push_str(spec);
        }
        out.push('}');
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    Ok(KeyTemplate {
        pattern: out,
        bindings,
        has_placeholder,
    })
}

/// 解析占位符名：函数参数 / `self` / `self.<字段路径>`
fn resolve_key_placeholder(
    name: &str,
    arg_names: &[&syn::Ident],
    span: proc_macro2::Span,
) -> syn::Result<proc_macro2::TokenStream> {
    if name == "self" {
        return Ok(quote! { self });
    }
    if let Some(path) = name.strip_prefix("self.") {
        let mut expr = quote! { self };
        for segment in path.split('.') {
            if segment.is_empty()
                || !segment.chars().all(|c| c.is_alphanumeric() || c == '_')
                || segment.chars().next().is_some_and(|c| c.is_numeric())
            {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "`#[cached]` key template placeholder `{{{name}}}` is not a valid `self.<field>` path"
                    ),
                ));
            }
            let ident = syn::Ident::new(segment, span);
            expr = quote! { #expr.#ident };
        }
        return Ok(expr);
    }
    if let Some(arg) = arg_names.iter().find(|arg| arg.to_string() == name) {
        return Ok(quote! { #arg });
    }
    Err(syn::Error::new(
        span,
        format!(
            "`#[cached]` key template placeholder `{{{name}}}` matches no parameter; use a parameter name, `self`, or a `self.<field>` path"
        ),
    ))
}

/// `cached` 的实现体（proc_macro2 输入输出）：与 proc-macro 入口分离，使
/// 各参数解析分支可被单元测试直接驱动，无需真实编译期调用。
fn cached_impl(
    args: proc_macro2::TokenStream,
    item: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    // Rule 12: surface parse failures as `compile_error!` with a span
    // pointing at the offending argument, instead of panicking.
    let args = match parser.parse2(args) {
        Ok(args) => args,
        Err(e) => return e.to_compile_error(),
    };
    let input = match syn::parse2::<ItemFn>(item) {
        Ok(parsed) => parsed,
        Err(e) => return e.to_compile_error(),
    };

    let mut service_name = "default".to_string();
    let mut ttl = quote! { None };
    let mut key_pattern = None;
    let mut key_prefix = None;
    let mut scope_expr = None;
    let mut sync_mode = false;
    let mut skip_cache_write = false;
    // new parameters
    let mut single_flight = false;
    let mut strict_mode = false;
    let mut condition_fn = None;
    let mut cache_none = false;
    let mut skip_idents: Vec<syn::Ident> = Vec::new();

    for arg in args {
        let arg_span = arg.span();
        match arg {
            // `sync` flag — boolean path-style argument (no value).
            Meta::Path(path) if path.is_ident("sync") => {
                sync_mode = true;
            }
            // `skip_cache_write` flag — boolean path-style argument (no value).
            Meta::Path(path) if path.is_ident("skip_cache_write") => {
                skip_cache_write = true;
            }
            // `single_flight` flag — enable concurrent miss dedup
            Meta::Path(path) if path.is_ident("single_flight") => {
                single_flight = true;
            }
            // `strict` flag — panic on unregistered cache name
            Meta::Path(path) if path.is_ident("strict") => {
                strict_mode = true;
            }
            // `cache_none` flag — cache None results (only meaningful when the
            // success type is `Option<T>`; consumed by the cache-write filter).
            Meta::Path(path) if path.is_ident("cache_none") => {
                cache_none = true;
            }
            // `skip(a, b)` — exclude named parameters from the default cache
            // key. Rejected in combination with `key` (explicit template makes
            // skip meaningless) and for unknown parameter names.
            Meta::List(list) if list.path.is_ident("skip") => {
                let parsed = match list
                    .parse_args_with(Punctuated::<syn::Ident, Token![,]>::parse_terminated)
                {
                    Ok(p) => p,
                    Err(e) => return e.to_compile_error(),
                };
                if parsed.is_empty() {
                    return syn::Error::new(
                        list.span(),
                        "`skip` expects at least one parameter name, e.g. `skip(password)`",
                    )
                    .to_compile_error()
                }
                skip_idents.extend(parsed);
            }
            Meta::NameValue(nv) => {
                let nv_span = nv.path.span();
                if nv.path.is_ident("service") {
                    match nv.value {
                        Expr::Lit(expr_lit) => match expr_lit.lit {
                            Lit::Str(lit) => service_name = lit.value(),
                            other => {
                                return syn::Error::new(
                                    other.span(),
                                    "`service` argument expects a string literal, e.g. `service = \"my_svc\"`",
                                )
                                .to_compile_error()
                            }
                        },
                        other => {
                            return syn::Error::new(
                                other.span(),
                                "`service` argument expects a string literal, e.g. `service = \"my_svc\"`",
                            )
                            .to_compile_error()
                        }
                    }
                } else if nv.path.is_ident("ttl") {
                    match nv.value {
                        Expr::Lit(expr_lit) => match expr_lit.lit {
                            Lit::Int(lit) => {
                                // Rule 12: surface parse failures (e.g. u64
                                // overflow) as `compile_error!` instead of
                                // panicking inside the proc-macro.
                                let val = match lit.base10_parse::<u64>() {
                                    Ok(v) => v,
                                    Err(e) => {
                                        return syn::Error::new(
                                            lit.span(),
                                            format!("invalid ttl value: {}", e),
                                        )
                                        .to_compile_error()
                                    }
                                };
                                ttl = quote! { Some(#val) };
                            }
                            other => {
                                return syn::Error::new(
                                    other.span(),
                                    "`ttl` argument expects an integer literal, e.g. `ttl = 60`",
                                )
                                .to_compile_error()
                            }
                        },
                        other => {
                            return syn::Error::new(
                                other.span(),
                                "`ttl` argument expects an integer literal, e.g. `ttl = 60`",
                            )
                            .to_compile_error()
                        }
                    }
                } else if nv.path.is_ident("key") {
                    match nv.value {
                        Expr::Lit(expr_lit) => match expr_lit.lit {
                            Lit::Str(lit) => key_pattern = Some(lit.value()),
                            other => {
                                return syn::Error::new(
                                    other.span(),
                                    "`key` argument expects a string literal, e.g. `key = \"user_{id}\"`",
                                )
                                .to_compile_error()
                            }
                        },
                        other => {
                            return syn::Error::new(
                                other.span(),
                                "`key` argument expects a string literal, e.g. `key = \"user_{id}\"`",
                            )
                            .to_compile_error()
                        }
                    }
                } else if nv.path.is_ident("key_prefix") {
                    match nv.value {
                        Expr::Lit(expr_lit) => match expr_lit.lit {
                            Lit::Str(lit) => key_prefix = Some(lit.value()),
                            other => {
                                return syn::Error::new(
                                    other.span(),
                                    "`key_prefix` argument expects a string literal, e.g. `key_prefix = \"ns\"`",
                                )
                                .to_compile_error()
                            }
                        },
                        other => {
                            return syn::Error::new(
                                other.span(),
                                "`key_prefix` argument expects a string literal, e.g. `key_prefix = \"ns\"`",
                            )
                            .to_compile_error()
                        }
                    }
                } else if nv.path.is_ident("condition") {
                    // condition = path_to_fn
                    match nv.value {
                        Expr::Path(expr_path) => {
                            condition_fn = Some(quote! { #expr_path });
                        }
                        other => {
                            return syn::Error::new(
                                other.span(),
                                "`condition` argument expects a function path, e.g. `condition = should_cache`",
                            )
                            .to_compile_error()
                        }
                    }
                } else if nv.path.is_ident("scope") {
                    // scope = <expr>：任意表达式，在函数体内求值；
                    // 方法可用 `scope = self.tenant_id` 承载租户 / 用户维度
                    scope_expr = Some(nv.value);
                } else {
                    // Rule 12: unknown NameValue argument — surface as
                    // compile_error instead of silent ignore.
                    let unknown = nv
                        .path
                        .get_ident()
                        .map(|i| i.to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    return syn::Error::new(
                        nv_span,
                        format!(
                            "unknown `#[cached]` argument `{}`; supported: service, ttl, key, key_prefix, scope, condition",
                            unknown
                        ),
                    )
                    .to_compile_error()
                }
            }
            // Rule 12: unsupported argument shape (e.g. Meta::List or unknown
            // path) — surface as compile_error instead of silent ignore.
            _ => {
                return syn::Error::new(
                    arg_span,
                    "unsupported `#[cached]` argument; supported: sync, skip_cache_write, single_flight, strict, cache_none, skip(param, ...), service = \"...\", ttl = N, key = \"...\", key_prefix = \"...\", scope = expr, condition = path",
                )
                .to_compile_error()
            }
        }
    }

    // Validate: `sync` cannot be combined with `async fn`. The sync branch
    // generates a plain `fn`, which is incompatible with `async` in the
    // user's signature. Rule 12: emit a `compile_error!` with a span
    // pointing at the attribute, instead of panicking.
    if sync_mode && input.sig.asyncness.is_some() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "`#[cached(sync)]` cannot be used with `async fn`; either remove `async` from the function signature or remove `sync` from the `#[cached]` arguments",
        )
        .to_compile_error();
    }

    let fn_name = &input.sig.ident;
    let fn_args = &input.sig.inputs;
    let fn_output = &input.sig.output;
    let fn_block = &input.block;
    let vis = &input.vis;

    // Extract the success type T from `Result<T, E>` (fall back to the whole
    // return type) and detect whether it is `Option<...>` — the latter drives
    // the `cache_none` write filter below.
    let success_ty: Option<&syn::Type> = match fn_output {
        syn::ReturnType::Default => None,
        syn::ReturnType::Type(_, ty) => {
            let mut found: Option<&syn::Type> = Some(ty);
            if let syn::Type::Path(path) = &**ty
                && let Some(seg) = path.path.segments.last()
                && seg.ident == "Result"
                && let syn::PathArguments::AngleBracketed(args) = &seg.arguments
                && let Some(syn::GenericArgument::Type(inner)) = args.args.first()
            {
                found = Some(inner);
            }
            found
        }
    };
    // For Result<T, E>, we need to extract T
    let return_type = match success_ty {
        Some(t) => quote! { #t },
        None => quote! { () },
    };
    let result_is_option = matches!(
        success_ty,
        Some(syn::Type::Path(p))
            if p.path.segments.last().is_some_and(|s| s.ident == "Option")
    );

    // Cache-write filter shared by the async, single-flight leader and sync
    // paths. With `T = Option<X>` and no `cache_none`, `Ok(None)` is not
    // written back (documented default: absent results are not cached); the
    // stored bytes are the serialized `Some` inner value, which deserializes
    // back through `Option<X>` transparently. With `cache_none`, `Ok(None)`
    // is written as `null` and restored as `Ok(None)` on read.
    let cache_write_block = |set_call: proc_macro2::TokenStream| {
        if result_is_option && !cache_none {
            quote! {
                if let ::std::result::Result::Ok(ref val) = result {
                    if let ::std::option::Option::Some(inner) = val {
                        if let Ok(bytes) = cache.unified_serializer().serialize(inner) {
                            let _ = #set_call;
                        }
                    }
                }
            }
        } else {
            quote! {
                if let ::std::result::Result::Ok(ref val) = result {
                    if let Ok(bytes) = cache.unified_serializer().serialize(val) {
                        let _ = #set_call;
                    }
                }
            }
        }
    };
    let async_cache_write = cache_write_block(quote! {
        cache.set_bytes(&cache_key, bytes, #ttl).await
    });
    let sync_cache_write = cache_write_block(quote! {
        cache.set_bytes_sync(&cache_key, bytes, #ttl)
    });

    // Generate argument names for key generation.
    // Rule 12: surface unsupported parameter shapes (destructuring) as
    // compile_error! instead of silently skipping — a skipped parameter
    // would silently weaken cache key uniqueness.
    let mut arg_names: Vec<&syn::Ident> = Vec::new();
    let mut receiver_span = None;
    for arg in fn_args.iter() {
        if let syn::FnArg::Typed(pat_type) = arg {
            match &*pat_type.pat {
                syn::Pat::Ident(pat_ident) => arg_names.push(&pat_ident.ident),
                other => {
                    return syn::Error::new(
                        other.span(),
                        "#[cached] does not support destructured parameters; use named parameters",
                    )
                    .to_compile_error();
                }
            }
        }
        // FnArg::Receiver (self) 不参与键：归属维度必须由 scope / key 显式承载
        if let syn::FnArg::Receiver(receiver) = arg {
            receiver_span = Some(receiver.span());
        }
    }

    // 带 self 接收者的函数：默认键不含任何归属维度，不同租户 / 用户会
    // 共享同一条缓存条目（跨租户串号）。因此作用域必须显式给出：
    // `scope = <expr>` 或显式 `key` 模板；缺失时编译期报错而非静默放行。
    if let Some(span) = receiver_span
        && scope_expr.is_none()
        && key_pattern.is_none()
    {
        return syn::Error::new(
            span,
            "`#[cached]` on a method with a `self` receiver requires `scope = <expr>` (e.g. `scope = self.tenant_id`) or an explicit `key = \"...\"` template; the default key carries no ownership dimension and would be shared across tenants/users",
        )
        .to_compile_error();
    }

    // 显式键模板：占位符绑定到函数参数或 `self` 字段路径。模板值逐段
    // 转义（与 KeyGenerator 同一口径），不同实参不别名到同一个键。
    let key_template = match key_pattern.as_deref() {
        Some(pattern) => match parse_key_template(pattern, &arg_names) {
            Ok(template) => Some(template),
            Err(err) => return err.to_compile_error(),
        },
        None => None,
    };

    // 带 self 接收者 + 常量键模板：模板逐字产出、与 self 无关，所有租户
    // 共用同一条缓存条目（跨租户串号）。模板必须承载归属维度——引用参数
    // 或 `self` 字段路径；否则编译期报错而非静默放行。
    if let (Some(span), Some(template)) = (receiver_span, key_template.as_ref())
        && !template.has_placeholder
    {
        return syn::Error::new(
            span,
            "`#[cached]` on a method with a `self` receiver requires the `key` template to reference the ownership dimension (e.g. `key = \"user_{self.tenant_id}\"`) or `scope = <expr>`; a constant template shares one cache entry across tenants/users",
        )
        .to_compile_error();
    }

    // Validate `skip` references (Rule 12): every skipped identifier must be
    // an actual function parameter, and `skip` must not be combined with an
    // explicit `key` template (the template fully determines the key, so
    // silently ignoring skip would conceal a configuration mistake).
    if !skip_idents.is_empty() && key_pattern.is_some() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "`#[cached]` `skip` cannot be combined with `key`; an explicit key template already determines the cache key, remove one of them",
        )
        .to_compile_error();
    }
    // 同理：`scope` 与显式键模板互斥（模板已完全决定键）
    if scope_expr.is_some() && key_pattern.is_some() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "`#[cached]` `scope` cannot be combined with `key`; an explicit key template already determines the cache key, remove one of them",
        )
        .to_compile_error();
    }
    for skipped in &skip_idents {
        let skipped_name = skipped.to_string();
        if !arg_names.iter().any(|arg| arg.to_string() == skipped_name) {
            return syn::Error::new(
                skipped.span(),
                format!(
                    "`skip` references `{}`, which is not a parameter of `{}`",
                    skipped, fn_name
                ),
            )
            .to_compile_error();
        }
    }

    // Key generation uses only non-skipped parameters; `condition` keeps
    // receiving the full argument list.
    let key_arg_names: Vec<&syn::Ident> = arg_names
        .iter()
        .filter(|arg| {
            let name = arg.to_string();
            !skip_idents.iter().any(|skipped| *skipped == name)
        })
        .copied()
        .collect();

    // Generate cloned argument names for key generation to avoid ownership issues
    let arg_names_cloned: Vec<_> = key_arg_names
        .iter()
        .map(|name| {
            quote! { (#name).clone() }
        })
        .collect();

    // 默认键的实参段：摘要而非 Debug 明文。
    // 实参常含口令 / 会话令牌，明文进入持久化键空间等于凭据落盘
    // （KEYS/SCAN、快照、MONITOR 可见）。
    let args_hash = if key_arg_names.is_empty() {
        None
    } else {
        Some(quote! {
            ::oxcache::macro_support::hash_key_args(&[&format!("{:?}", (#(#arg_names_cloned),*))])
        })
    };

    // 作用域段：表达式在函数体内求值（方法可用 self.tenant_id）。
    // 分隔符转义防作用域值伪造段边界（与 KeyGenerator 同一口径）
    let scope_value = scope_expr.as_ref().map(|expr| {
        quote! {
            {
                let __oxcache_scope = ::oxcache::macro_support::escape_key_segment(
                    &::std::string::ToString::to_string(&(#expr)),
                );
                __oxcache_scope
            }
        }
    });

    // Generate key logic with cloned args
    let key_gen_with_cloned_args = if let Some(template) = key_template.as_ref() {
        // 显式键模板：占位符值经段转义后按模板产出
        let pattern = syn::LitStr::new(&template.pattern, proc_macro2::Span::call_site());
        let bindings = template.bindings.iter().map(|(ident, value)| {
            quote! {
                let #ident = ::oxcache::macro_support::escape_key_segment(&#value);
            }
        });
        let args = template
            .bindings
            .iter()
            .map(|(ident, _)| quote! { #ident = #ident });
        quote! {
            {
                #(#bindings)*
                ::std::format!(#pattern, #(#args),*)
            }
        }
    } else if let Some(prefix) = key_prefix {
        // Use key_prefix with default generation
        match (&scope_value, &args_hash) {
            (Some(scope), Some(hash)) => quote! {
                format!("{}:{}:{}:{}:{}", #service_name, #prefix, #scope, stringify!(#fn_name), #hash)
            },
            (Some(scope), None) => quote! {
                format!("{}:{}:{}:{}", #service_name, #prefix, #scope, stringify!(#fn_name))
            },
            (None, Some(hash)) => quote! {
                format!("{}:{}:{}:{}", #service_name, #prefix, stringify!(#fn_name), #hash)
            },
            (None, None) => quote! {
                format!("{}:{}:{}", #service_name, #prefix, stringify!(#fn_name))
            },
        }
    } else {
        // Default key generation: service[:scope]:fn_name[:args-hash]
        // (skipped parameters excluded)
        match (&scope_value, &args_hash) {
            (Some(scope), Some(hash)) => quote! {
                format!("{}:{}:{}:{}", #service_name, #scope, stringify!(#fn_name), #hash)
            },
            (Some(scope), None) => quote! {
                format!("{}:{}:{}", #service_name, #scope, stringify!(#fn_name))
            },
            (None, Some(hash)) => quote! {
                format!("{}:{}:{}", #service_name, stringify!(#fn_name), #hash)
            },
            (None, None) => quote! {
                format!("{}:{}", #service_name, stringify!(#fn_name))
            },
        }
    };

    // condition check generation
    let condition_check = if let Some(cond) = &condition_fn {
        let args_pass: Vec<_> = arg_names.iter().map(|n| quote! { #n.clone() }).collect();
        quote! {
            if !#cond(#(#args_pass),*) {
                // Condition false: bypass cache entirely
                return { #fn_block };
            }
        }
    } else {
        quote! {}
    };

    let condition_check_async = if let Some(cond) = &condition_fn {
        let args_pass: Vec<_> = arg_names.iter().map(|n| quote! { #n.clone() }).collect();
        quote! {
            if !#cond(#(#args_pass),*) {
                return async { #fn_block }.await;
            }
        }
    } else {
        quote! {}
    };

    // strict mode — cache lookup failure handling
    // The panic message is rendered through oxcache's Fluent i18n catalog
    // (key `macro-service-not-registered`), resolving under the user's locale.
    let cache_miss_handler = if strict_mode {
        quote! {
            None => panic!(
                "{}",
                ::oxcache::i18n::messages::t(
                    "macro.service_not_registered",
                    &[("service", #service_name.to_string())],
                )
            ),
        }
    } else {
        quote! {
            None => {
                ::oxcache::__telemetry_macro_passthrough(#service_name, "service_not_registered");
                return { #fn_block };
            }
        }
    };

    let cache_miss_handler_async = if strict_mode {
        quote! {
            None => panic!(
                "{}",
                ::oxcache::i18n::messages::t(
                    "macro.service_not_registered",
                    &[("service", #service_name.to_string())],
                )
            ),
        }
    } else {
        quote! {
            None => {
                ::oxcache::__telemetry_macro_passthrough(#service_name, "service_not_registered");
                return async { #fn_block }.await;
            }
        }
    };

    // single_flight — generate per-function sharded static lock map
    let sf_static = if single_flight && !sync_mode {
        let sf_locks_name = syn::Ident::new(
            &format!("__OXCACHE_SF_{}", fn_name.to_string().to_uppercase()),
            fn_name.span(),
        );
        quote! {
            static #sf_locks_name: ::std::sync::LazyLock<
                [::oxcache::macro_support::SfShard; ::oxcache::macro_support::SF_SHARDS]
            > = ::std::sync::LazyLock::new(|| ::std::array::from_fn(|_| ::std::sync::Mutex::new(::std::collections::HashMap::new())));
        }
    } else {
        quote! {}
    };

    let sf_locks_name = syn::Ident::new(
        &format!("__OXCACHE_SF_{}", fn_name.to_string().to_uppercase()),
        fn_name.span(),
    );

    let sf_logic_async = if single_flight {
        quote! {
            // Single-flight（审计 F03/F04 修复）：64 路分片 + watch flight 信号
            // + panic 守卫。follower 在分片锁内 subscribe（owned Receiver 可带出
            // 锁），watch 版本比对语义使 "leader 先完成、follower 后等待" 仍立即
            // 返回，不存在 Notify::notify_waiters 的丢失唤醒窗口。
            enum __OxcacheFlight {
                Leader(::std::sync::Arc<tokio::sync::watch::Sender<()>>),
                Follower(tokio::sync::watch::Receiver<()>),
            }
            let __flight = {
                let __idx = ::oxcache::macro_support::shard_index(&cache_key);
                let mut __map = #sf_locks_name[__idx].lock().unwrap();
                match __map.entry(cache_key.clone()) {
                    ::std::collections::hash_map::Entry::Occupied(e) => {
                        __OxcacheFlight::Follower(e.get().subscribe())
                    }
                    ::std::collections::hash_map::Entry::Vacant(e) => {
                        let (__tx, _rx) = tokio::sync::watch::channel(());
                        let __tx = ::std::sync::Arc::new(__tx);
                        e.insert(__tx.clone());
                        __OxcacheFlight::Leader(__tx)
                    }
                }
            };

            match __flight {
                __OxcacheFlight::Follower(mut __rx) => {
                    ::oxcache::macro_support::wait_flight(__rx).await;
                    // Re-check cache after leader completes
                    if let Ok(Some(bytes)) = cache.get_bytes(&cache_key).await {
                        if let Ok(val) = cache.unified_serializer().deserialize::<#return_type>(&bytes) {
                            return ::std::result::Result::Ok(val);
                        }
                    }
                    // Leader failed to cache — run locally
                    return async { #fn_block }.await;
                }
                __OxcacheFlight::Leader(__signal) => {
                    // panic 安全守卫：panic / 早退时 Drop 兜底移除条目并放行等待者，
                    // 该 key 后续调用可重新成为 leader（不产生 key 级永久死锁）
                    let mut __guard = ::oxcache::macro_support::AsyncSfGuard::new(
                        &#sf_locks_name,
                        ::oxcache::macro_support::shard_index(&cache_key),
                        cache_key.clone(),
                        __signal,
                    );

                    // Leader path: execute + cache + signal
                    let result = async { #fn_block }.await;

                    if !#skip_cache_write {
                        #async_cache_write
                    }

                    __guard.finish();
                    return result;
                }
            }
        }
    } else {
        quote! {}
    };

    if sync_mode {
        // Sync branch
        quote! {
            #vis fn #fn_name(#fn_args) #fn_output {
                #condition_check

                let cache_key = #key_gen_with_cloned_args;

                let cache = match ::oxcache::__internal_get_cache(#service_name) {
                    Some(c) => c,
                    #cache_miss_handler
                };

                if let Ok(Some(bytes)) = cache.get_bytes_sync(&cache_key) {
                    if let Ok(val) = cache.unified_serializer().deserialize::<#return_type>(&bytes) {
                        return ::std::result::Result::Ok(val);
                    }
                }

                let result = { #fn_block };

                if !#skip_cache_write {
                    #sync_cache_write
                }

                result
            }
        }
    } else {
        // Async branch
        quote! {
            #sf_static

            #[allow(unreachable_code)]
            #vis async fn #fn_name(#fn_args) #fn_output {
                #condition_check_async

                let cache_key = #key_gen_with_cloned_args;

                let cache = match ::oxcache::__internal_get_cache(#service_name) {
                    Some(c) => c,
                    #cache_miss_handler_async
                };

                #sf_logic_async

                // Non-single-flight path
                if let Ok(Some(bytes)) = cache.get_bytes(&cache_key).await {
                    if let Ok(val) = cache.unified_serializer().deserialize::<#return_type>(&bytes) {
                        return ::std::result::Result::Ok(val);
                    }
                }

                let result = async { #fn_block }.await;

                if !#skip_cache_write {
                    #async_cache_write
                }

                result
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::cached_impl;

    /// 展开为 Display 文本（token 间以空格分隔，字符串字面量内容原样保留）。
    fn expand(args: &str, item: &str) -> String {
        let args: proc_macro2::TokenStream = args.parse().expect("args tokenstream");
        let item: proc_macro2::TokenStream = item.parse().expect("item tokenstream");
        cached_impl(args, item).to_string()
    }

    /// 压缩全部空白：Display 在 token 间插入空格，压缩后便于断言相邻 token 片段。
    fn compact(expanded: &str) -> String {
        expanded.chars().filter(|c| !c.is_whitespace()).collect()
    }

    fn assert_compile_error(expanded: &str, fragment: &str) {
        assert!(
            expanded.contains("compile_error"),
            "expected compile_error, got: {expanded}"
        );
        assert!(
            expanded.contains(fragment),
            "message fragment `{fragment}` missing in: {expanded}"
        );
    }

    const FN_PLAIN: &str = "fn f(x: u32) -> String { format!(\"{}\", x) }";

    #[test]
    fn sync_and_flag_args_generate_read_path() {
        let expanded = expand("sync, skip_cache_write", FN_PLAIN);
        assert!(!expanded.contains("compile_error"));
        assert!(expanded.contains("get_bytes_sync"), "{expanded}");
        // skip_cache_write 的写回以常量 false 守卫（token 仍生成，运行期恒跳过）
        assert!(expanded.contains("if ! true"), "{expanded}");
    }

    #[test]
    fn async_path_without_flags_generates_cache_write() {
        let expanded = expand("", FN_PLAIN);
        assert!(!expanded.contains("compile_error"));
        assert!(expanded.contains("get_bytes"), "{expanded}");
        assert!(expanded.contains("set_bytes"), "{expanded}");
    }

    #[test]
    fn skip_list_rejects_non_ident() {
        let expanded = expand("skip(1)", FN_PLAIN);
        assert_compile_error(&expanded, "expected identifier");
    }

    #[test]
    fn skip_rejects_empty_list() {
        let expanded = expand("skip()", FN_PLAIN);
        assert_compile_error(&expanded, "`skip` expects at least one parameter name");
    }

    #[test]
    fn service_rejects_non_literal_expr() {
        let expanded = expand("service = svc_name()", FN_PLAIN);
        assert_compile_error(&expanded, "`service` argument expects a string literal");
    }

    #[test]
    fn service_rejects_non_string_literal() {
        let expanded = expand("service = 42", FN_PLAIN);
        assert_compile_error(&expanded, "`service` argument expects a string literal");
    }

    #[test]
    fn ttl_rejects_integer_overflow() {
        let expanded = expand("ttl = 99999999999999999999999", FN_PLAIN);
        assert_compile_error(&expanded, "invalid ttl value");
    }

    #[test]
    fn ttl_rejects_string_literal() {
        let expanded = expand("ttl = \"60\"", FN_PLAIN);
        assert_compile_error(&expanded, "`ttl` argument expects an integer literal");
    }

    #[test]
    fn ttl_rejects_non_literal_expr() {
        let expanded = expand("ttl = ttl_expr", FN_PLAIN);
        assert_compile_error(&expanded, "`ttl` argument expects an integer literal");
    }

    #[test]
    fn key_rejects_non_string_literal() {
        let expanded = expand("key = 42", FN_PLAIN);
        assert_compile_error(&expanded, "`key` argument expects a string literal");
    }

    #[test]
    fn key_rejects_non_literal_expr() {
        let expanded = expand("key = key_expr", FN_PLAIN);
        assert_compile_error(&expanded, "`key` argument expects a string literal");
    }

    #[test]
    fn key_prefix_rejects_non_string_literal() {
        let expanded = expand("key_prefix = 42", FN_PLAIN);
        assert_compile_error(&expanded, "`key_prefix` argument expects a string literal");
    }

    #[test]
    fn key_prefix_rejects_non_literal_expr() {
        let expanded = expand("key_prefix = prefix_expr", FN_PLAIN);
        assert_compile_error(&expanded, "`key_prefix` argument expects a string literal");
    }

    #[test]
    fn condition_rejects_non_path() {
        let expanded = expand("condition = \"check\"", FN_PLAIN);
        assert_compile_error(&expanded, "`condition` argument expects a function path");
    }

    #[test]
    fn unknown_named_argument_rejected() {
        let expanded = expand("unknown_arg = 1", FN_PLAIN);
        assert_compile_error(&expanded, "unknown `#[cached]` argument `unknown_arg`");
    }

    #[test]
    fn unsupported_argument_shape_rejected() {
        let expanded = expand("bogus", FN_PLAIN);
        assert_compile_error(&expanded, "unsupported `#[cached]` argument");
    }

    #[test]
    fn default_return_type_maps_to_unit_deserialization() {
        let expanded = compact(&expand("sync", "fn f() {}"));
        assert!(!expanded.contains("compile_error"), "{expanded}");
        assert!(expanded.contains("deserialize::<()>"), "{expanded}");
    }

    #[test]
    fn destructured_parameter_rejected() {
        let expanded = expand("", "fn f((a, b): (u32, u32)) -> u32 { a }");
        assert_compile_error(&expanded, "does not support destructured parameters");
    }

    #[test]
    fn non_function_item_rejected() {
        let expanded = expand("", "struct S;");
        assert_compile_error(&expanded, "expected `fn`");
    }

    #[test]
    fn receiver_parameter_requires_scope_or_key() {
        // 带 self 接收者且无作用域 → 编译期报错（默认键不含归属维度）
        let expanded = expand("", "fn m(&self, k: u32) -> u32 { k }");
        assert_compile_error(&expanded, "requires `scope = <expr>`");
    }

    #[test]
    fn receiver_with_scope_is_accepted() {
        let expanded = expand("scope = self.tenant_id", "fn m(&self, k: u32) -> u32 { k }");
        assert!(!expanded.contains("compile_error"), "{expanded}");
        assert!(expanded.contains("get_bytes"), "{expanded}");
        // 作用域表达式进入键
        assert!(expanded.contains("self . tenant_id"), "{expanded}");
    }

    #[test]
    fn receiver_with_explicit_key_is_accepted() {
        let expanded = expand("key = \"{self:?}:{k}\"", "fn m(&self, k: u32) -> u32 { k }");
        assert!(!expanded.contains("compile_error"), "{expanded}");
    }

    #[test]
    fn scope_conflicts_with_key_template() {
        let expanded = expand(
            "scope = self.tenant_id, key = \"{k}\"",
            "fn m(&self, k: u32) -> u32 { k }",
        );
        assert_compile_error(&expanded, "`scope` cannot be combined with `key`");
    }

    /// 默认键的实参段必须是摘要，不得把 Debug 明文写进键
    #[test]
    fn default_key_hashes_arguments_instead_of_debug_text() {
        let expanded = compact(&expand("", "fn f(a: u64, b: u64) -> u64 { a + b }"));
        assert!(!expanded.contains("compile_error"), "{expanded}");
        assert!(
            expanded.contains("hash_key_args"),
            "default key must hash arguments: {expanded}"
        );
        // 旧的明文键格式（把 Debug 串直接拼进键）不得再出现
        assert!(
            !expanded.contains("\"{}:{}:{:?}\"") && !expanded.contains("\"{}:{}:{}:{:?}\""),
            "default key must not embed Debug-formatted arguments: {expanded}"
        );
    }

    #[test]
    fn key_template_binds_parameter_placeholder() {
        let expanded = compact(&expand(
            "key = \"user_{id}\"",
            "fn load(id: u64) -> String { String::new() }",
        ));
        assert!(!expanded.contains("compile_error"), "{expanded}");
        // 占位符改写为绑定名，绑定值经段转义后入键
        assert!(expanded.contains("__oxcache_key_arg_0"), "{expanded}");
        assert!(expanded.contains("escape_key_segment"), "{expanded}");
    }

    #[test]
    fn key_template_binds_self_field_path() {
        let expanded = compact(&expand(
            "key = \"user_{self.tenant_id}\"",
            "fn load(&self) -> String { String::new() }",
        ));
        assert!(!expanded.contains("compile_error"), "{expanded}");
        assert!(expanded.contains("self.tenant_id"), "{expanded}");
    }

    /// 常量键模板在方法上不承载归属维度：编译期报错（跨租户串号）
    #[test]
    fn constant_key_template_on_method_rejected() {
        let expanded = expand("key = \"user_profile\"", "fn m(&self) -> u32 { 1 }");
        assert_compile_error(&expanded, "reference the ownership dimension");
    }

    /// 常量键模板在自由函数上允许（无租户维度）
    #[test]
    fn constant_key_template_on_free_function_allowed() {
        let expanded = expand("key = \"user_profile\"", "fn f() -> u32 { 1 }");
        assert!(!expanded.contains("compile_error"), "{expanded}");
    }

    #[test]
    fn unknown_key_placeholder_rejected() {
        let expanded = expand("key = \"user_{nope}\"", "fn f(id: u64) -> u64 { id }");
        assert_compile_error(&expanded, "matches no parameter");
    }

    #[test]
    fn key_prefix_without_args_uses_fn_name_only() {
        let expanded = compact(&expand(
            "key_prefix = \"ns\"",
            "fn ping() -> String { String::new() }",
        ));
        assert!(!expanded.contains("compile_error"), "{expanded}");
        assert!(
            expanded.contains("format!(\"{}:{}:{}\",\"default\",\"ns\",stringify!(ping))"),
            "{expanded}"
        );
    }
}
