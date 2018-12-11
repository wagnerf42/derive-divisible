extern crate proc_macro;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

#[proc_macro_derive(Divisible, attributes(divide_by))]
pub fn derive_divisible(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    // implement base_length
    let len_expression = generate_len_expression(&input.data);

    // split into tuple of couples (left and right)
    let split_expression = generate_split_declarations(&input.data);
    // move tuple into fields of split structure
    let left_fields = generate_fields(&input.data, 0);
    let right_fields = generate_fields(&input.data, 1);

    let expanded = quote! {
        impl #impl_generics Divisible for #name #ty_generics #where_clause {
            fn base_length(&self) -> usize {
                #len_expression
            }
            fn divide(self) -> (Self, Self) {
                #split_expression
                (
                    #name {
                        #left_fields
                    },
                    #name{
                        #right_fields
                    }
                )
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

/// Fill fields of target struct from content of tuple storing
/// split fields.
/// Index indicate if we fill left or right structure.
fn generate_fields(data: &Data, index: usize) -> TokenStream {
    match *data {
        Data::Struct(ref data) => {
            match data.fields {
                Fields::Named(ref fields) => {
                    let recurse = fields.named.iter().enumerate().map(|(i, f)| {
                        let name = &f.ident;
                        quote! {
                            #name: (split_fields.#i).#index
                        }
                    });
                    quote! {
                        #(#recurse, )*
                    }
                }
                Fields::Unnamed(ref fields) => {
                    let recurse = fields.unnamed.iter().enumerate().map(|(i, _)| {
                        quote! {
                            (split_fields.#i).#index
                        }
                    });
                    quote! {
                        #(#recurse, )*
                    }
                }
                Fields::Unit => {
                    // Unit structs have a base length of 0.
                    quote!()
                }
            }
        }
        Data::Enum(_) | Data::Union(_) => unimplemented!(),
    }
}

/// Generate the function splitting the divisible
fn generate_split_declarations(data: &Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => {
            match data.fields {
                Fields::Named(ref fields) => {
                    let recurse = fields.named.iter().map(|f| {
                        let name = &f.ident;
                        match find_strategy(&f) {
                            DivideBy::Copy => {
                                quote! {
                                    (self.#name, self.#name)
                                }
                            }
                            DivideBy::Default => {
                                quote! {
                                    (self.#name, Default::default())
                                }
                            }
                            DivideBy::Divisible => {
                                quote! {
                                    self.#name.divide()
                                }
                            }
                        }
                    });
                    quote! {
                        let split_fields = (#(#recurse, )*);
                    }
                }
                Fields::Unnamed(ref fields) => {
                    let recurse =
                        fields
                            .unnamed
                            .iter()
                            .enumerate()
                            .map(|(i, f)| match find_strategy(&f) {
                                DivideBy::Copy => {
                                    quote! {
                                        (self.#i, self.#i)
                                    }
                                }
                                DivideBy::Default => {
                                    quote! {
                                        (self.#i, Default::default())
                                    }
                                }
                                DivideBy::Divisible => {
                                    quote! {
                                        self.#i.divide()
                                    }
                                }
                            });
                    quote! {
                        let split_fields = (#(#recurse, )*);
                    }
                }
                Fields::Unit => {
                    // Unit structs have a base length of 0.
                    quote!()
                }
            }
        }
        Data::Enum(_) | Data::Union(_) => unimplemented!(),
    }
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
                    let recurse = fields
                        .unnamed
                        .iter()
                        .enumerate()
                        .filter(|&(_, f)| find_strategy(f) == DivideBy::Divisible)
                        .map(|(i, _)| {
                            quote! {::std::iter::once(self.#i.len())}
                        });
                    quote! {
                        ::std::iter::once(0)#(.chain(#recurse))*.max().unwrap()
                    }
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
