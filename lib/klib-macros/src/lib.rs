#![feature(log_syntax)]

extern crate proc_macro;
extern crate proc_macro2;
extern crate quote;
extern crate syn;

mod error;

use std::str::FromStr;

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{spanned::Spanned, DataStruct, DeriveInput, Field, Fields, FieldsNamed};

use crate::error::MacroError;

fn editor_name(item: &DeriveInput) -> Result<TokenStream, MacroError> {
  let name = &item.ident;

  Ok(TokenStream::from_str(&format!("{}Editor", name))?.into())
}

fn editor_for_struct(item: &DeriveInput, s: &DataStruct) -> Result<TokenStream, MacroError> {
  let name = item.ident.to_token_stream();
  let fields = if let Fields::Named(fields) = s.fields {
    fields
  } else {
    return Err(MacroError::Message(
      "EditableConfig only valid on named structs",
      item.span(),
    ));
  };

	for f in &fields.named {
		f.ty.
	}
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
