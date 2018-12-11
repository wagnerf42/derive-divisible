extern crate proc_macro;

use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, parse_quote, Data, DeriveInput, Fields, GenericParam, Generics, Index,
};

#[proc_macro_derive(Divisible, attributes(divide_by))]
pub fn derive_divisible(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let len_expression = generate_len_expression(&input.data);
    let expanded = quote! {
        impl #impl_generics Divisible for #name #ty_generics #where_clause {
            fn length(&self) -> usize {
                #len_expression
            }
        }
    };
    proc_macro::TokenStream::from(expanded)
}

/// What strategy to apply when dividing a field.
#[derive(Debug, PartialEq, Eq)]
enum DivideBy {
    /// Copy the field
    Copy,
    /// Take a default value
    Default,
    /// Divide using divisible
    Divisible,
}

/// figure out what division strategy to use for a given field.
fn find_strategy(field: &syn::Field) -> DivideBy {
    // loop on all attributes
    field
        .attrs
        .as_slice()
        .into_iter()
        .filter(|a| {
            // only the first "divide_by" attribute is interesting
            let i = syn::Ident::new("divide_by", proc_macro2::Span::call_site());
            a.path.is_ident(i)
        })
        .next()
        .map(|a| {
            // look further into the group of arguments
            let possible_group: Result<proc_macro2::Group, _> = syn::parse2(a.tts.clone());
            possible_group
                .ok()
                .and_then(|g| {
                    // we only care about first argument
                    let possible_id_token = g.stream().into_iter().next();
                    possible_id_token.map(|token| match token {
                        proc_macro2::TokenTree::Ident(i) => {
                            let ident = i.to_string();
                            if ident == "copy" {
                                DivideBy::Copy
                            } else if ident == "default" {
                                DivideBy::Default
                            } else {
                                DivideBy::Divisible
                            }
                        }
                        _ => DivideBy::Divisible,
                    })
                })
                .unwrap_or(DivideBy::Divisible)
        })
        .unwrap_or(DivideBy::Divisible)
}

/// compute base length of the structure
fn generate_len_expression(data: &Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => {
            match data.fields {
                Fields::Named(ref fields) => {
                    let recurse = fields
                        .named
                        .iter()
                        .filter(|f| find_strategy(f) == DivideBy::Divisible)
                        .map(|f| {
                            let name = &f.ident;
                            quote! {::std::iter::once(self.#name.len())}
                        });
                    quote! {
                        ::std::iter::once(0)#(.chain(#recurse))*.max().unwrap()
                    }
                }
                Fields::Unnamed(ref fields) => {
                    unimplemented!()
                    //                    // Expands to an expression like
                    //                    //
                    //                    //     0 + self.0.heap_size() + self.1.heap_size() + self.2.heap_size()
                    //                    let recurse = fields.unnamed.iter().enumerate().map(|(i, f)| {
                    //                        let index = Index::from(i);
                    //                        quote_spanned! {f.span()=>
                    //                            ::heapsize::HeapSize::heap_size_of_children(&self.#index)
                    //                        }
                    //                    });
                    //                    quote! {
                    //                        0 #(+ #recurse)*
                    //                    }
                }
                Fields::Unit => {
                    // Unit structs have a base length of 0.
                    quote!(0)
                }
            }
        }
        Data::Enum(_) | Data::Union(_) => unimplemented!(),
    }
}
