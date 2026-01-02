//! Copyright (c) 2025-2026, Kirky.X
//!
//! MIT License
//!
//! 该模块定义了oxcache的宏实现，提供缓存注解功能。

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::Parser, parse_macro_input, punctuated::Punctuated, Expr, ItemFn, Lit, Meta, Token,
};

#[proc_macro_attribute]
pub fn cached(args: TokenStream, item: TokenStream) -> TokenStream {
    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let args = parser.parse(args).expect("Failed to parse arguments");
    let input = parse_macro_input!(item as ItemFn);

    let mut service_name = "default".to_string();
    let mut ttl = quote! { None };
    let mut key_pattern = None;
    let mut cache_type = quote! { "two-level" };

    for arg in args {
        if let Meta::NameValue(nv) = arg {
            if nv.path.is_ident("service") {
                if let Expr::Lit(expr_lit) = nv.value {
                    if let Lit::Str(lit) = expr_lit.lit {
                        service_name = lit.value();
                    }
                }
            } else if nv.path.is_ident("ttl") {
                if let Expr::Lit(expr_lit) = nv.value {
                    if let Lit::Int(lit) = expr_lit.lit {
                        let val = lit.base10_parse::<u64>().unwrap();
                        ttl = quote! { Some(#val) };
                    }
                }
            } else if nv.path.is_ident("key") {
                if let Expr::Lit(expr_lit) = nv.value {
                    if let Lit::Str(lit) = expr_lit.lit {
                        key_pattern = Some(lit.value());
                    }
                }
            } else if nv.path.is_ident("cache_type") {
                if let Expr::Lit(expr_lit) = nv.value {
                    if let Lit::Str(lit) = expr_lit.lit {
                        let val = lit.value();
                        cache_type = quote! { #val };
                    }
                }
            }
        }
    }

    let fn_name = &input.sig.ident;
    let fn_args = &input.sig.inputs;
    let fn_output = &input.sig.output;
    let fn_block = &input.block;
    let vis = &input.vis;

    // Generate key logic
    let key_gen = if let Some(pattern) = key_pattern {
        // We allow the user to use the format string syntax directly, e.g. "user_{id}" where id is an arg.
        // This works because we are in the scope of the function arguments.
        quote! {
            format!(#pattern)
        }
    } else {
        // Default key generation: service:fn_name:arg1:arg2...
        // We need to capture argument names.
        let arg_names: Vec<_> = fn_args
            .iter()
            .filter_map(|arg| {
                if let syn::FnArg::Typed(pat_type) = arg {
                    if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                        return Some(&pat_ident.ident);
                    }
                }
                None
            })
            .collect();

        if arg_names.is_empty() {
            quote! { format!("{}:{}", #service_name, stringify!(#fn_name)) }
        } else {
            quote! {
                format!("{}:{}:{:?}", #service_name, stringify!(#fn_name), (#(#arg_names),*))
            }
        }
    };

    let output = quote! {
        #vis async fn #fn_name(#fn_args) #fn_output {
            use oxcache::{get_client, CacheOps};

            let cache_key = #key_gen;

            // Try to get client, if fails, run original function
            let client = match get_client(#service_name) {
                Ok(c) => c,
                Err(_) => return async { #fn_block }.await,
            };

            // Try get from cache
            // We use the client's internal serializer (via CacheOps::serializer()) to handle serialization.
            if let Ok(Some(bytes)) = client.get_bytes(&cache_key).await {
                 use oxcache::serialization::Serializer;
                 if let Ok(val) = client.serializer().deserialize(&bytes) {
                     return Ok(val);
                 }
            }

            // Run original function
            let result = async { #fn_block }.await;

            // Cache result if Ok
            if let Ok(ref val) = result {
                 use oxcache::serialization::Serializer;
                 if let Ok(bytes) = client.serializer().serialize(val) {
                    let _ = match #cache_type {
                        "l1-only" => client.set_l1_bytes(&cache_key, bytes, #ttl).await,
                        "l2-only" => client.set_l2_bytes(&cache_key, bytes, #ttl).await,
                        _ => client.set_bytes(&cache_key, bytes, #ttl).await,
                    };
                 }
            }

            result
        }
    };

    output.into()
}
