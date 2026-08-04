// Copyright (c) 2025-2026 Kirky.X
// SPDX-License-Identifier: MIT
//! 该模块定义了oxcache的宏实现，提供缓存注解功能。

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse::Parser, parse_macro_input, punctuated::Punctuated, spanned::Spanned, Expr, ItemFn, Lit, Meta, Token};

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
    // T003: when true, the macro skips the cache-write side effect for the
    // `Ok` path (i.e. even successful results are NOT written to the cache).
    // Default `false` preserves the existing behavior (Ok results are cached).
    // Renamed from `skip_errors` (misleading) to `skip_cache_write` (accurate).
    let mut skip_cache_write = false;

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
                                        return syn::Error::new(lit.span(), format!("invalid ttl value: {}", e))
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
                            "unknown `#[cached]` argument `{}`; supported: service, ttl, key, key_prefix",
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
                    "unsupported `#[cached]` argument; supported: sync, skip_cache_write, service = \"...\", ttl = N, key = \"...\", key_prefix = \"...\"",
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

    let output = if sync_mode {
        // Sync branch: generate a plain `fn` (no `async`). Uses
        // `get_bytes_sync` / `set_bytes_sync` which require the registered
        // cache to have been built with `sync_mode(true)`.
        quote! {
            #vis fn #fn_name(#fn_args) #fn_output {
                let cache_key = #key_gen_with_cloned_args;

                // Try to get cache instance, if fails, run original function
                let cache = match ::oxcache::__internal_get_cache(#service_name) {
                    Some(c) => c,
                    None => return { #fn_block },
                };

                // Try get from cache using sync byte-level operations.
                // Deserialize failure is treated as cache miss (silent fallback).
                if let Ok(Some(bytes)) = cache.get_bytes_sync(&cache_key) {
                    if let Ok(val) = cache.unified_serializer().deserialize::<#return_type>(&bytes) {
                        return ::std::result::Result::Ok(val);
                    }
                }

                // Run original function
                let result = { #fn_block };

                // Cache result if Ok — skipped when `skip_cache_write` is set.
                // Serialize/write failures are silently ignored (result still returned).
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
        // Async branch (default, original behavior)
        quote! {
            #vis async fn #fn_name(#fn_args) #fn_output {
                let cache_key = #key_gen_with_cloned_args;

                // Try to get cache instance, if fails, run original function
                let cache = match ::oxcache::__internal_get_cache(#service_name) {
                    Some(c) => c,
                    None => return async { #fn_block }.await,
                };

                // Try get from cache using byte-level operations.
                // Deserialize failure is treated as cache miss (silent fallback).
                if let Ok(Some(bytes)) = cache.get_bytes(&cache_key).await {
                    if let Ok(val) = cache.unified_serializer().deserialize::<#return_type>(&bytes) {
                        return ::std::result::Result::Ok(val);
                    }
                }

                // Run original function
                let result = async { #fn_block }.await;

                // Cache result if Ok — skipped when `skip_cache_write` is set.
                // Serialize/write failures are silently ignored (result still returned).
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
