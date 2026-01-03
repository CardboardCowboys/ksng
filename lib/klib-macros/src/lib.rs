#![feature(log_syntax)]

extern crate proc_macro;
extern crate proc_macro2;
extern crate quote;
extern crate syn;

mod error;

use std::str::FromStr;

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{DataStruct, DeriveInput, Field, Fields, FieldsNamed, spanned::Spanned};

use crate::error::MacroError;

fn editor_name(item: &DeriveInput) -> Result<TokenStream, MacroError> {
  let name = &item.ident;

  Ok(TokenStream::from_str(&format!("{}Editor", name))?.into())
}

fn editor_for_struct(item: &DeriveInput, s: &DataStruct) -> Result<TokenStream, MacroError> {
  let name = item.ident.to_token_stream();
  let fields = if let Fields::Named(fields) = &s.fields {
    fields
  } else {
    return Err(MacroError::Message(
      "EditableConfig only valid on named structs",
      item.span(),
    ));
  };

  todo!();
}

fn editable_config_impl(item: DeriveInput) -> Result<TokenStream, MacroError> {
  let visitor = match &item.data {
    syn::Data::Struct(s) => editor_for_struct(&item, s),
    // TODO: editor for enum
    _ => Err(MacroError::Message(
      "EditableConfig is only valid for structs",
      item.span(),
    )),
  }?;

  todo!()
}

#[proc_macro_derive(EditableConfig, attributes())]
pub fn editable_config(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
  let original: TokenStream = input.clone().into();
  let item: DeriveInput = syn::parse(input).unwrap();
  match editable_config_impl(item) {
    Ok(stream) => quote! {
      #original
      #stream
    }
    .into(),
    Err(err) => err.to_token_stream().into(),
  }
}

fn editable_enum_impl(item: DeriveInput) -> Result<TokenStream, MacroError> {
  let syn::Data::Enum(enum_val) = &item.data else {
    return Err(MacroError::Message(
      "EditableEnum only valid for enums",
      item.span(),
    ));
  };

  let name = item.ident.to_token_stream();
  let items: Vec<TokenStream> = enum_val
    .variants
    .iter()
    .map(|v| v.ident.to_token_stream())
    .collect();

  let items_str: Vec<TokenStream> = items.iter().map(|i| quote! { "#(i)" }).collect();

  let items_trait = quote! {
    impl EditableEnum for #name {
      fn items() -> &'static [&'static str] {
        &[#(#items_str),*]
      }
    }
  };

  Ok(items_trait)
}

#[proc_macro_derive(EditableEnum, attributes())]
pub fn editable_enum(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
  let original: TokenStream = input.clone().into();
  let item: DeriveInput = syn::parse(input).unwrap();
  match editable_config_impl(item) {
    Ok(stream) => quote! {
      #original
      #stream
    }
    .into(),
    Err(err) => err.to_token_stream().into(),
  }
}
