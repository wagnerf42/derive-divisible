//! This crate provides automatic derivation for `rayon-adaptive`
//! divisibility traits. If you don't know them you should go there first.
//! By default it will just divide all fields but you can use attributes to specify
//! two different behaviors.
//! `clone` will instead clone the field to get the same value on both sides and
//! `default` will keep the value on the left side and reset the value on a default value
//! on the right side.
extern crate proc_macro;

use proc_macro2::{Group, TokenStream};
use quote::quote;
use syn::{parse_macro_input, Attribute, Data, DeriveInput, Fields};

#[proc_macro_derive(Divisible, attributes(divide_by, power))]
pub fn derive_divisible(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let power = power_type(&input.attrs);
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
            type Power = #power;
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

/// Return argument of first attribute with given name.
fn attributes_search(attributes: &[Attribute], searched_attribute_name: &str) -> Option<Group> {
    attributes
        .into_iter()
        .find(|a| {
            let i = syn::Ident::new(searched_attribute_name, proc_macro2::Span::call_site());
            a.path.is_ident(i)
        })
        .and_then(|a| {
            // look further into the group of arguments
            let possible_group: Result<proc_macro2::Group, _> = syn::parse2(a.tts.clone());
            possible_group.ok()
        })
}

/// Extract power attribute's value
fn power_type(attributes: &[Attribute]) -> Group {
    attributes_search(attributes, "power").expect("missing power attribute")
}

#[proc_macro_derive(DivisibleIntoBlocks, attributes(divide_by))]
pub fn derive_divisible_into_blocks(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // split into tuple of couples (left and right)
    let split_expression = generate_split_into_blocks_declarations(&input.data);
    // move tuple into fields of split structure
    let left_fields = generate_fields(&input.data, 0);
    let right_fields = generate_fields(&input.data, 1);

    let expanded = quote! {
        impl #impl_generics DivisibleIntoBlocks for #name #ty_generics #where_clause {
            fn divide_at(self, index: usize) -> (Self, Self) {
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

#[proc_macro_derive(DivisibleAtIndex, attributes(divide_by))]
pub fn derive_divisible_at_index(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let expanded = quote! {
        impl #impl_generics DivisibleAtIndex for #name #ty_generics #where_clause {}
    };
    proc_macro::TokenStream::from(expanded)
}

/// What strategy to apply when dividing a field.
#[derive(Debug, PartialEq, Eq)]
enum DivideBy {
    /// Clone the field
    Clone,
    /// Take a default value on right side and move on the left
    Default,
    /// Divide using divisible
    Divisible,
}

/// figure out what division strategy to use for a given field.
fn find_strategy(field: &syn::Field) -> DivideBy {
    attributes_search(&field.attrs, "divide_by")
        .map(|group| {
            let string = group
                .stream()
                .into_iter()
                .map(|s| s.to_string())
                .collect::<String>();
            match string.as_ref() {
                "clone" => DivideBy::Clone,
                "default" => DivideBy::Default,
                _ => DivideBy::Divisible,
            }
        })
        .unwrap_or(DivideBy::Divisible)
}

/// Fill fields of target struct from content of tuple storing
/// split fields.
/// Index indicate if we fill left or right structure.
fn generate_fields(data: &Data, index: usize) -> TokenStream {
    let index = syn::Index::from(index);
    match *data {
        Data::Struct(ref data) => match data.fields {
            Fields::Named(ref fields) => {
                let recurse = fields.named.iter().enumerate().map(|(i, f)| {
                    let i = syn::Index::from(i);
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
                    let i = syn::Index::from(i);
                    quote! {
                        (split_fields.#i).#index
                    }
                });
                quote! {
                    #(#recurse, )*
                }
            }
            Fields::Unit => quote!(),
        },
        Data::Enum(_) | Data::Union(_) => unimplemented!(),
    }
}

/// Generate the function splitting the divisible
fn generate_split_declarations(data: &Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => match data.fields {
            Fields::Named(ref fields) => {
                let recurse = fields.named.iter().map(|f| {
                    let name = &f.ident;
                    match find_strategy(&f) {
                        DivideBy::Clone => {
                            quote! {
                                (self.#name.clone(), self.#name)
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
                let recurse = fields.unnamed.iter().enumerate().map(|(i, f)| {
                    let i = syn::Index::from(i);
                    match find_strategy(&f) {
                        DivideBy::Clone => {
                            quote! {
                                (self.#i.clone(), self.#i)
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
                    }
                });
                quote! {
                    let split_fields = (#(#recurse, )*);
                }
            }
            Fields::Unit => quote!(),
        },
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
                            quote! {::std::iter::once(self.#name.base_length())}
                        });
                    quote! {
                        ::std::iter::once(std::usize::MAX)#(.chain(#recurse))*.min().unwrap()
                    }
                }
                Fields::Unnamed(ref fields) => {
                    let recurse = fields
                        .unnamed
                        .iter()
                        .enumerate()
                        .filter(|&(_, f)| find_strategy(f) == DivideBy::Divisible)
                        .map(|(i, _)| {
                            quote! {::std::iter::once(self.#i.base_length())}
                        });
                    quote! {
                        ::std::iter::once(std::usize::MAX)#(.chain(#recurse))*.min().unwrap()
                    }
                }
                Fields::Unit => {
                    // Unit structs have an infinite base length
                    quote!(std::usize::MAX)
                }
            }
        }
        Data::Enum(_) | Data::Union(_) => unimplemented!(),
    }
}

/// Generate the function splitting the divisible
fn generate_split_into_blocks_declarations(data: &Data) -> TokenStream {
    match *data {
        Data::Struct(ref data) => match data.fields {
            Fields::Named(ref fields) => {
                let recurse = fields.named.iter().map(|f| {
                    let name = &f.ident;
                    match find_strategy(&f) {
                        DivideBy::Clone => {
                            quote! {
                                (self.#name.clone(), self.#name)
                            }
                        }
                        DivideBy::Default => {
                            quote! {
                                (self.#name, Default::default())
                            }
                        }
                        DivideBy::Divisible => {
                            quote! {
                                self.#name.divide_at(index)
                            }
                        }
                    }
                });
                quote! {
                    let split_fields = (#(#recurse, )*);
                }
            }
            Fields::Unnamed(ref fields) => {
                let recurse = fields.unnamed.iter().enumerate().map(|(i, f)| {
                    let i = syn::Index::from(i);
                    match find_strategy(&f) {
                        DivideBy::Clone => {
                            quote! {
                                (self.#i.clone(), self.#i)
                            }
                        }
                        DivideBy::Default => {
                            quote! {
                                (self.#i, Default::default())
                            }
                        }
                        DivideBy::Divisible => {
                            quote! {
                                self.#i.divide_at(index)
                            }
                        }
                    }
                });
                quote! {
                    let split_fields = (#(#recurse, )*);
                }
            }
            Fields::Unit => quote!(),
        },
        Data::Enum(_) | Data::Union(_) => unimplemented!(),
    }
}
