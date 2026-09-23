// Copyright (c) 2025-2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
//! 该模块定义了oxcache的宏实现，提供缓存注解功能。

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Expr, ItemFn, Lit, Meta, Token, parse::Parser, parse_macro_input, punctuated::Punctuated,
    spanned::Spanned,
};

#[proc_macro_attribute]
pub fn cached(args: TokenStream, item: TokenStream) -> TokenStream {
    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    // Rule 12: surface parse failures as `compile_error!` with a span
    // pointing at the offending argument, instead of panicking.
    let args = match parser.parse(args) {
        Ok(args) => args,
        Err(e) => return e.to_compile_error().into(),
    };
    let input = parse_macro_input!(item as ItemFn);

    let mut service_name = "default".to_string();
    let mut ttl = quote! { None };
    let mut key_pattern = None;
    let mut key_prefix = None;
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
                    Err(e) => return e.to_compile_error().into(),
                };
                if parsed.is_empty() {
                    return syn::Error::new(
                        list.span(),
                        "`skip` expects at least one parameter name, e.g. `skip(password)`",
                    )
                    .to_compile_error()
                    .into();
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
                                .into();
                            }
                        },
                        other => {
                            return syn::Error::new(
                                other.span(),
                                "`service` argument expects a string literal, e.g. `service = \"my_svc\"`",
                            )
                            .to_compile_error()
                            .into();
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
                                        .into();
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
                                .into();
                            }
                        },
                        other => {
                            return syn::Error::new(
                                other.span(),
                                "`ttl` argument expects an integer literal, e.g. `ttl = 60`",
                            )
                            .to_compile_error()
                            .into();
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
                                .into();
                            }
                        },
                        other => {
                            return syn::Error::new(
                                other.span(),
                                "`key` argument expects a string literal, e.g. `key = \"user_{id}\"`",
                            )
                            .to_compile_error()
                            .into();
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
                                .into();
                            }
                        },
                        other => {
                            return syn::Error::new(
                                other.span(),
                                "`key_prefix` argument expects a string literal, e.g. `key_prefix = \"ns\"`",
                            )
                            .to_compile_error()
                            .into();
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
                            .into();
                        }
                    }
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
                            "unknown `#[cached]` argument `{}`; supported: service, ttl, key, key_prefix, condition",
                            unknown
                        ),
                    )
                    .to_compile_error()
                    .into();
                }
            }
            // Rule 12: unsupported argument shape (e.g. Meta::List or unknown
            // path) — surface as compile_error instead of silent ignore.
            _ => {
                return syn::Error::new(
                    arg_span,
                    "unsupported `#[cached]` argument; supported: sync, skip_cache_write, single_flight, strict, cache_none, skip(param, ...), service = \"...\", ttl = N, key = \"...\", key_prefix = \"...\", condition = path",
                )
                .to_compile_error()
                .into();
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
        .to_compile_error()
        .into();
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
    for arg in fn_args.iter() {
        if let syn::FnArg::Typed(pat_type) = arg {
            match &*pat_type.pat {
                syn::Pat::Ident(pat_ident) => arg_names.push(&pat_ident.ident),
                other => {
                    return syn::Error::new(
                        other.span(),
                        "#[cached] does not support destructured parameters; use named parameters",
                    )
                    .to_compile_error()
                    .into();
                }
            }
        }
        // FnArg::Receiver (self) is silently skipped — not part of cache key.
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
        .to_compile_error()
        .into();
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
            .to_compile_error()
            .into();
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

    // Generate key logic with cloned args
    let key_gen_with_cloned_args = if let Some(pattern) = key_pattern {
        // Custom format string pattern: "user_{id}"
        quote! {
            format!(#pattern)
        }
    } else if let Some(prefix) = key_prefix {
        // Use key_prefix with default generation
        if key_arg_names.is_empty() {
            quote! { format!("{}:{}:{}", #service_name, #prefix, stringify!(#fn_name)) }
        } else {
            quote! {
                format!("{}:{}:{}:{:?}", #service_name, #prefix, stringify!(#fn_name), (#(#arg_names_cloned),*))
            }
        }
    } else {
        // Default key generation: service:fn_name:arg1:arg2... (skipped
        // parameters excluded)
        if key_arg_names.is_empty() {
            quote! { format!("{}:{}", #service_name, stringify!(#fn_name)) }
        } else {
            quote! {
                format!("{}:{}:{:?}", #service_name, stringify!(#fn_name), (#(#arg_names_cloned),*))
            }
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

    let output = if sync_mode {
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
    };

    output.into()
}
