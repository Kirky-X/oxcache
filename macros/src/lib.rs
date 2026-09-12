// Copyright (c) 2025-2026 Kirky.X
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
    let mut _cache_none = false;

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
            // `cache_none` flag — cache None results
            Meta::Path(path) if path.is_ident("cache_none") => {
                _cache_none = true;
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
                    "unsupported `#[cached]` argument; supported: sync, skip_cache_write, single_flight, strict, cache_none, service = \"...\", ttl = N, key = \"...\", key_prefix = \"...\", condition = path",
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

    // Extract return type from fn_output for type annotations
    // For Result<T, E>, we need to extract T
    let return_type = match fn_output {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => {
            // Try to extract T from Result<T, E>
            if let syn::Type::Path(path) = &**ty {
                if let Some(seg) = path.path.segments.last() {
                    if seg.ident == "Result" {
                        if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                            if let Some(first_arg) = args.args.first() {
                                quote! { #first_arg }
                            } else {
                                quote! { #ty }
                            }
                        } else {
                            quote! { #ty }
                        }
                    } else {
                        quote! { #ty }
                    }
                } else {
                    quote! { #ty }
                }
            } else {
                quote! { #ty }
            }
        }
    };

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

    // Generate cloned argument names for key generation to avoid ownership issues
    let arg_names_cloned: Vec<_> = arg_names
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
        if arg_names.is_empty() {
            quote! { format!("{}:{}:{}", #service_name, #prefix, stringify!(#fn_name)) }
        } else {
            quote! {
                format!("{}:{}:{}:{:?}", #service_name, #prefix, stringify!(#fn_name), (#(#arg_names_cloned),*))
            }
        }
    } else {
        // Default key generation: service:fn_name:arg1:arg2...
        if arg_names.is_empty() {
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
    let cache_miss_handler = if strict_mode {
        quote! {
            None => panic!("oxcache: service '{}' not registered (strict mode)", #service_name),
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
            None => panic!("oxcache: service '{}' not registered (strict mode)", #service_name),
        }
    } else {
        quote! {
            None => {
                ::oxcache::__telemetry_macro_passthrough(#service_name, "service_not_registered");
                return async { #fn_block }.await;
            }
        }
    };

    // single_flight — generate per-function static lock map
    let sf_static = if single_flight && !sync_mode {
        let sf_locks_name = syn::Ident::new(
            &format!("__OXCACHE_SF_{}", fn_name.to_string().to_uppercase()),
            fn_name.span(),
        );
        quote! {
            static #sf_locks_name: ::std::sync::LazyLock<
                ::std::sync::Mutex<::std::collections::HashMap<String, ::std::sync::Arc<tokio::sync::Notify>>>
            > = ::std::sync::LazyLock::new(|| ::std::sync::Mutex::new(::std::collections::HashMap::new()));
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
            // Single-flight: register as leader or become follower
            let (is_follower, notify) = {
                let mut map = #sf_locks_name.lock().unwrap();
                match map.entry(cache_key.clone()) {
                    ::std::collections::hash_map::Entry::Occupied(e) => (true, e.get().clone()),
                    ::std::collections::hash_map::Entry::Vacant(e) => {
                        let n = ::std::sync::Arc::new(tokio::sync::Notify::new());
                        e.insert(n.clone());
                        (false, n)
                    }
                }
            };

            if is_follower {
                notify.notified().await;
                // Re-check cache after leader completes
                if let Ok(Some(bytes)) = cache.get_bytes(&cache_key).await {
                    if let Ok(val) = cache.unified_serializer().deserialize::<#return_type>(&bytes) {
                        return ::std::result::Result::Ok(val);
                    }
                }
                // Leader failed to cache — run locally
                return async { #fn_block }.await;
            }

            // Leader path: execute + cache + notify
            let result = async { #fn_block }.await;

            if !#skip_cache_write {
                if let Ok(ref val) = result {
                    if let Ok(bytes) = cache.unified_serializer().serialize(val) {
                        let _ = cache.set_bytes(&cache_key, bytes, #ttl).await;
                    }
                }
            }

            // Notify followers and clean up
            {
                let mut map = #sf_locks_name.lock().unwrap();
                map.remove(&cache_key);
            }
            notify.notify_waiters();

            return result;
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
                    if let Ok(ref val) = result {
                        if let Ok(bytes) = cache.unified_serializer().serialize(val) {
                            let _ = cache.set_bytes_sync(&cache_key, bytes, #ttl);
                        }
                    }
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
                    if let Ok(ref val) = result {
                        if let Ok(bytes) = cache.unified_serializer().serialize(val) {
                            let _ = cache.set_bytes(&cache_key, bytes, #ttl).await;
                        }
                    }
                }

                result
            }
        }
    };

    output.into()
}
