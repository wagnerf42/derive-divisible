//! This crate provides automatic derivation for `rayon-adaptive`
//! divisibility traits. If you don't know them you should go there first.
//! By default it will just divide all fields but you can use attributes to specify
//! two different behaviors.
//! `clone` will instead clone the field to get the same value on both sides and
//! `default` will keep the value on the left side and reset the value on a default value
//! on the right side.
#![recursion_limit = "256"]
extern crate proc_macro;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Attribute, Data, DeriveInput, Fields};

#[proc_macro_derive(Divisible, attributes(divide_by, power, trait_bounds))]
pub fn derive_divisible(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let power = attributes_search(&input.attrs, "power").expect("missing power attribute");
    let name = input.ident;
    let bounds = attributes_search(&input.attrs, "trait_bounds");
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let proto = bounds // user given bounds override automatic bounds
        .map(|b| {
            quote! {impl <#b> Divisible for #name #ty_generics} // TODO: why the where clause ?
        })
        .unwrap_or(quote! {impl #impl_generics Divisible for #name #ty_generics #where_clause});

    // implement base_length
    let len_expression = generate_len_expression(&input.data);

    // split into tuple of couples (left and right)
    let split_expression = generate_split_declarations(&input.data);
    // move tuple into fields of split structure
    let left_fields = generate_fields(&input.data, 0);
    let right_fields = generate_fields(&input.data, 1);

    let expanded = quote! {
        #proto {
            type Power = #power;
            fn base_length(&self) -> Option<usize> {
                #len_expression
            }
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

/// Return argument of first attribute with given name.
fn attributes_search(
    attributes: &[Attribute],
    searched_attribute_name: &str,
) -> Option<TokenStream> {
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
        .map(|g| g.stream())
}

/// What strategy to apply when dividing a field.
#[derive(Debug, PartialEq, Eq)]
enum DivideBy {
    /// Clone the field
    Clone,
    /// Copy the field (mainly for functions which implement Copy but not Clone)
    Copy,
    /// Take a default value on right side and move on the left
    Default,
    /// Divide using divisible
    Divisible,
}

/// find divisible field if only one.
fn divisible_content(data: &Data) -> Option<TokenStream> {
    match *data {
        Data::Struct(ref data) => match data.fields {
            Fields::Named(ref fields) => {
                let mut names = fields
                    .named
                    .iter()
                    .filter(|f| find_strategy(f) == DivideBy::Divisible)
                    .map(|f| {
                        let name = &f.ident;
                        quote! {#name}
                    });
                let first_name = names.next();
                let second_name = names.next();
                if first_name.is_none() || second_name.is_some() {
                    None
                } else {
                    first_name
                }
            }
            Fields::Unnamed(ref fields) => {
                let mut names = fields
                    .unnamed
                    .iter()
                    .enumerate()
                    .filter(|&(_, f)| find_strategy(f) == DivideBy::Divisible)
                    .map(|(i, _)| {
                        quote! {#i}
                    });
                let first_name = names.next();
                let second_name = names.next();
                if first_name.is_none() || second_name.is_some() {
                    None
                } else {
                    first_name
                }
            }
            Fields::Unit => None,
        },
        Data::Enum(_) | Data::Union(_) => unimplemented!(),
    }
}

/// figure out what division strategy to use for a given field.
fn find_strategy(field: &syn::Field) -> DivideBy {
    attributes_search(&field.attrs, "divide_by")
        .map(|stream| {
            let string = stream
                .into_iter()
                .map(|s| s.to_string())
                .collect::<String>();
            match string.as_ref() {
                "clone" => DivideBy::Clone,
                "default" => DivideBy::Default,
                "copy" => DivideBy::Copy,
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
                        ::std::iter::once(Some(std::usize::MAX))#(.chain(#recurse))*.filter_map(|s| s).min()
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
                        ::std::iter::once(Some(std::usize::MAX))#(.chain(#recurse))*.filter_map(|s| s).min()
                    }
                }
                Fields::Unit => {
                    // Unit structs have an infinite base length
                    quote!(Some(std::usize::MAX))
                }
            }
        }
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

// now implement ParallelIterator

#[proc_macro_derive(
    ParallelIterator,
    attributes(
        divide_by,
        power,
        item,
        sequential_iterator,
        iterator_extraction,
        trait_bounds
    )
)]
pub fn derive_parallel_iterator(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let item = attributes_search(&input.attrs, "item").expect("missing item attribute");
    let bounds = attributes_search(&input.attrs, "trait_bounds");
    let sequential_iterator = attributes_search(&input.attrs, "sequential_iterator")
        .expect("missing sequential_iterator attribute");
    let iterator_extraction = attributes_search(&input.attrs, "iterator_extraction")
        .expect("missing iterator_extraction attribute");
    let name = input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let proto = bounds // user given bounds override automatic bounds
        .map(|b| {
            quote! {impl <#b> ParallelIterator for #name #ty_generics} // TODO: why the where clause ?
        })
        .unwrap_or(
            quote! {impl #impl_generics ParallelIterator for #name #ty_generics #where_clause},
        );

    let inner_iterator =
        divisible_content(&input.data).expect("we could not find only one iterator inside us");
    let expanded = quote! {
        #proto {
            type SequentialIterator = #sequential_iterator;
            type Item = #item;
            fn extract_iter(&mut self, size: usize) -> Self::SequentialIterator {
                let i = self.#inner_iterator.extract_iter(size);
                #iterator_extraction
            }
            fn to_sequential(self) -> Self::SequentialIterator {
                let i = self.#inner_iterator.to_sequential();
                #iterator_extraction
            }
            fn blocks_sizes(&mut self) -> Box<Iterator<Item=usize>> {
                self.#inner_iterator.blocks_sizes()
            }
            fn policy(&self) -> crate::Policy {
                self.#inner_iterator.policy()
            }
        }

    };

    proc_macro::TokenStream::from(expanded)
}
