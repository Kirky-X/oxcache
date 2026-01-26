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
    let mut key_prefix = None;
    let mut key_generator_type = "default".to_string();
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
            } else if nv.path.is_ident("key_prefix") {
                if let Expr::Lit(expr_lit) = nv.value {
                    if let Lit::Str(lit) = expr_lit.lit {
                        key_prefix = Some(lit.value());
                    }
                }
            } else if nv.path.is_ident("key_generator") {
                if let Expr::Lit(expr_lit) = nv.value {
                    if let Lit::Str(lit) = expr_lit.lit {
                        key_generator_type = lit.value();
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

    // Generate argument names for key generation
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



    // Generate cloned argument names for key generation to avoid ownership issues
    let arg_names_cloned: Vec<_> = arg_names.iter().map(|name| {
        quote! { (#name).clone() }
    }).collect();
    
    // Generate key logic with cloned args
    let key_gen_with_cloned_args = if let Some(pattern) = key_pattern {
        // Custom format string pattern: "user_{id}"
        quote! {
            format!(#pattern)
        }
    } else if key_generator_type != "default" {
        // Use KeyGenerator for structured key generation
        match key_generator_type.as_str() {
            "simple" => {
                if arg_names.is_empty() {
                    quote! {
                        oxcache::KeyGenerator::simple(#service_name, stringify!(#fn_name))
                    }
                } else {
                    quote! {
                        oxcache::KeyGenerator::simple_with_args(
                            #service_name,
                            stringify!(#fn_name),
                            &(#(#arg_names_cloned),*)
                        )
                    }
                }
            }
            "md5" => {
                if arg_names.is_empty() {
                    quote! {
                        oxcache::KeyGenerator::md5(#service_name, stringify!(#fn_name), "")
                    }
                } else {
                    quote! {
                        oxcache::KeyGenerator::md5_with_args(
                            #service_name,
                            stringify!(#fn_name),
                            &(#(#arg_names_cloned),*)
                        )
                    }
                }
            }
            "murmur3" => {
                if arg_names.is_empty() {
                    quote! {
                        oxcache::KeyGenerator::murmur3(#service_name, stringify!(#fn_name), "")
                    }
                } else {
                    quote! {
                        oxcache::KeyGenerator::murmur3_with_args(
                            #service_name,
                            stringify!(#fn_name),
                            &(#(#arg_names_cloned),*)
                        )
                    }
                }
            }
            "namespace" => {
                if arg_names.is_empty() {
                    quote! {
                        oxcache::KeyGenerator::namespace(
                            #service_name,
                            #key_prefix.unwrap_or("default"),
                            stringify!(#fn_name),
                            ""
                        )
                    }
                } else {
                    quote! {
                        oxcache::KeyGenerator::namespace_with_args(
                            #service_name,
                            #key_prefix.unwrap_or("default"),
                            stringify!(#fn_name),
                            &(#(#arg_names_cloned),*)
                        )
                    }
                }
            }
            _ => {
                // Fallback to default format
                if arg_names.is_empty() {
                    quote! { format!("{}:{}", #service_name, stringify!(#fn_name)) }
                } else {
                    quote! {
                        format!("{}:{}:{:?}", #service_name, stringify!(#fn_name), (#(#arg_names_cloned),*))
                    }
                }
            }
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
    
    let output = quote! {
        #vis async fn #fn_name(#fn_args) #fn_output {
            let cache_key = #key_gen_with_cloned_args;

            // Validate cache key length and characters using library's validation function
            if let Err(e) = ::oxcache::utils::validate_cache_key(&cache_key) {
                tracing::warn!(
                    "Invalid cache key: {}. Falling back to uncached execution",
                    e
                );
                return async { #fn_block }.await;
            }

            // Try to get client, if fails, run original function
            let client = match ::oxcache::__internal_get_cache(#service_name) {
                Some(c) => c,
                None => return async { #fn_block }.await,
            };

            // Try get from cache
            // We use the client's internal serializer (via CacheOps::serializer()) to handle serialization.
            if let Ok(Some(bytes)) = client.get_bytes(&cache_key).await {
                 use ::oxcache::serialization::Serializer;
                 if let Ok(val) = client.serializer().deserialize::<#return_type>(&bytes) {
                     return ::std::result::Result::Ok(val);
                 }
            }

            // Run original function
            let result = async { #fn_block }.await;

            // Cache result if Ok
            if let Ok(ref val) = result {
                 use ::oxcache::serialization::Serializer;
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
