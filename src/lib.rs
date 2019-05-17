//! This crate provides automatic derivation for `rayon-adaptive`
//! divisibility traits. If you don't know them you should go there first.
//! By default it will just divide all fields but you can use attributes to specify
//! two different behaviors.
//! `clone` will instead clone the field to get the same value on both sides and
//! `default` will keep the value on the left side and reset the value on a default value
//! on the right side.
#![recursion_limit = "256"]
extern crate proc_macro;

use proc_macro2::{Group, TokenStream};
use quote::quote;
use syn::{parse_macro_input, Attribute, Data, DeriveInput, Fields};

#[proc_macro_derive(Divisible, attributes(divide_by, power))]
pub fn derive_divisible(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let power = attributes_search(&input.attrs, "power").expect("missing power attribute");
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
        impl #impl_generics Divisible<#power> for #name #ty_generics #where_clause {
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
                        ::std::iter::once(Some(std::usize::MAX))#(.chain(#recurse))*.min().unwrap()
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
                        ::std::iter::once(Some(std::usize::MAX))#(.chain(#recurse))*.min().unwrap()
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

// now implement ParallelIterator

#[proc_macro_derive(
    ParallelIterator,
    attributes(divide_by, power, item, sequential_iterator, iterator_extraction)
)]
pub fn derive_parallel_iterator(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let power = attributes_search(&input.attrs, "power").expect("missing power attribute");
    let item = attributes_search(&input.attrs, "item").expect("missing item attribute");
    let sequential_iterator = attributes_search(&input.attrs, "sequential_iterator")
        .expect("missing sequential_iterator attribute");
    let iterator_extraction = attributes_search(&input.attrs, "iterator_extraction")
        .expect("missing iterator_extraction attribute");
    let name = input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let inner_iterator =
        divisible_content(&input.data).expect("we could not find only one iterator inside us");
    let expanded = quote! {
        impl #impl_generics ParallelIterator<#power> for #name #ty_generics #where_clause {
            type SequentialIterator = #sequential_iterator;
            type Item = #item;
            fn iter(mut self, size: usize) -> (Self::SequentialIterator, Self) {
                let (i, remaining) = self.#inner_iterator.iter(size);
                self.#inner_iterator = remaining;
                (#iterator_extraction, self)
            }
            fn blocks_sizes(&mut self) -> Box<Iterator<Item=usize>> {
                self.#inner_iterator.blocks_sizes()
            }
            fn policy(&self) -> Policy {
                self.#inner_iterator.policy()
            }
        }

    };

    proc_macro::TokenStream::from(expanded)
}

// now implement IntoIterator
#[proc_macro_derive(IntoIterator, attributes(divide_by, power, item))]
pub fn derive_intoiterator(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let power = attributes_search(&input.attrs, "power").expect("missing power attribute");
    let item = attributes_search(&input.attrs, "item").expect("missing item attribute");
    let name = input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let expanded = quote! {
            impl #impl_generics IntoIterator for #name #ty_generics #where_clause {
            type Item = #item;
            type IntoIter = std::iter::FlatMap<
                    crate::divisibility::BlocksIterator<#power, Self, Box<Iterator<Item = usize>>>,
                    std::iter::Flatten<std::collections::linked_list::IntoIter<Vec<Self::Item>>>,
                    fn(Self) -> std::iter::Flatten<std::collections::linked_list::IntoIter<Vec<Self::Item>>>,
            >;
            fn into_iter(mut self) -> Self::IntoIter {
                let sizes = self.blocks_sizes();
                self.blocks(sizes).flat_map(|b| {
                    b.fold(Vec::new, |mut v, e| {
                        v.push(e);
                        v
                    }).map(|v| std::iter::once(v).collect::<std::collections::LinkedList<Vec<Self::Item>>>())
                    .reduce(std::collections::LinkedList::new, |mut l1, mut l2| {
                        l1.append(&mut l2);
                        l1
                    }).into_iter().flatten()
                })
            }
        }
    };

    proc_macro::TokenStream::from(expanded)
}
