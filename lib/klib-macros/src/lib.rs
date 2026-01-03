#![feature(log_syntax)]

extern crate proc_macro;
extern crate proc_macro2;
extern crate quote;
extern crate syn;

mod error;

use std::str::FromStr;

use proc_macro2::{Literal, TokenStream};
use quote::{ToTokens, quote};
use syn::{
  Attribute, DataEnum, DataStruct, DeriveInput, Field, Fields, FieldsNamed, Ident, LitFloat,
  LitInt, Token, parse::Parse, punctuated::Punctuated, spanned::Spanned, token::Comma,
};

use crate::error::MacroError;

struct SliderArgs(LitFloat, LitFloat);

impl Parse for SliderArgs {
  fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
    let args = Punctuated::<LitFloat, Comma>::parse_terminated(input)?;
    if args.len() != 2 {
      return Err(input.error("#[slider(min, max)] attribute requires two arguments"));
    }

    Ok(SliderArgs(args[0].clone(), args[1].clone()))
  }
}

struct FloatArgs(TokenStream, TokenStream);

impl Parse for FloatArgs {
  fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
    let args = Punctuated::<LitFloat, Comma>::parse_terminated(input)?;
    if args.is_empty() {
      Ok(FloatArgs(quote! { None }, quote! { None }))
    } else if args.len() == 1 {
      let min = &args[0];
      Ok(FloatArgs(quote! { Some(#min) }, quote! { None }))
    } else if args.len() == 2 {
      let min = &args[0];
      let max = &args[1];
      Ok(FloatArgs(quote! { Some(#min) }, quote! { Some(#max) }))
    } else {
      Err(input.error("#[number] attribute takes zero to two arguments"))
    }
  }
}

struct IntegerArgs(TokenStream, TokenStream);

impl Parse for IntegerArgs {
  fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
    let args = Punctuated::<LitInt, Comma>::parse_terminated(input)?;
    if args.is_empty() {
      Ok(IntegerArgs(quote! { None }, quote! { None }))
    } else if args.len() == 1 {
      let min = &args[0];
      Ok(IntegerArgs(quote! { Some(#min) }, quote! { None }))
    } else if args.len() == 2 {
      let min = &args[0];
      let max = &args[1];
      Ok(IntegerArgs(quote! { Some(#min) }, quote! { Some(#max) }))
    } else {
      Err(input.error("#[integer] attribute takes zero to two arguments"))
    }
  }
}

fn find_attr_named<'a>(attrs: &'a [Attribute], name: &'static str) -> Option<&'a Attribute> {
  for attr in attrs {
    let Some(ident) = attr.meta.path().get_ident() else {
      continue;
    };

    if *ident == name {
      return Some(attr);
    }
  }

  None
}

fn editor_for_field(
  redeclare: bool,
  inline: bool,
  field_ident: Ident,
  field: &Field,
) -> Result<TokenStream, MacroError> {
  let field_name = field_ident.to_string();
  let access = if redeclare {
    quote! { #field_ident }
  } else {
    quote! { self.#field_ident }
  };
  let set = if redeclare {
    quote! { let #field_ident }
  } else {
    quote! { self.#field_ident }
  };
  let key = if inline {
    quote! { "" }
  } else {
    quote! { #field_name }
  };

  match &field.ty {
    syn::Type::Path(type_path) => {
      let type_name = type_path.path.segments.last().ok_or(MacroError::Message(
        "Type has no path segments",
        field.span(),
      ))?;
      let type_ident = type_name.ident.clone();
      let type_name = type_name.ident.to_string();
      if type_name == "bool" {
        return Ok(quote! {
          #set = (ui.checkbox)(#key, &mut changed, #access);
        });
      } else if type_name == "f32" {
        if let Some(slider) = find_attr_named(&field.attrs, "slider") {
          let args: SliderArgs = slider.parse_args()?;
          let min = args.0;
          let max = args.1;
          return Ok(quote! {
            #set = (ui.slider)(#key, &mut changed, #min, #max, #access);
          });
        } else if let Some(float) = find_attr_named(&field.attrs, "float") {
          let args: FloatArgs = float.parse_args()?;
          let min = args.0;
          let max = args.1;
          return Ok(quote! {
            #set = (ui.float)(#key, &mut changed, #min, #max, #access);
          });
        }

        return Ok(quote! {
          #set = (ui.float)(#key, &mut changed, None, None, #access);
        });
      } else if type_name == "i32" || type_name == "i64" || type_name == "usize" {
        if let Some(integer) = find_attr_named(&field.attrs, "integer") {
          let args: IntegerArgs = integer.parse_args()?;
          let min = args.0;
          let max = args.1;
          return Ok(quote! {
            #set = (ui.integer)(#key, &mut changed, #min, #max, #access as i64) as #type_ident;
          });
        }

        return Ok(quote! {
          #set = (ui.integer)(#key, &mut changed, None, None, #access as i64) as #type_ident;
        });
      } else if type_name == "Rect" {
        return Ok(quote! {
          #set = (ui.normalized_rect)(#key, &mut changed, #access);
        });
      } else if type_name == "Timecode" {
        return Ok(quote! {
          #set = (ui.timecode)(#key, &mut changed, #access);
        });
      } else if type_name == "Font" {
        return Ok(quote! {
          #set = (ui.font)(#key, &mut changed, #access.clone());
        });
      } else if type_name == "Color32" {
        return Ok(quote! {
          #set = (ui.color)(#key, &mut changed, #access);
        });
      }

      let new_name = format!("new_{}", field_name);
      let new_name = Ident::new(&new_name, field_ident.span());

      Ok(quote! {
        let mut #new_name = #access.clone();
        (ui.config)(#key, &mut changed, &mut #new_name);
        #set = #new_name;
      })
    }
    _ => Err(MacroError::Message("Unsupported field type", field.span())),
  }
}

fn editor_for_struct(item: &DeriveInput, s: &DataStruct) -> Result<TokenStream, MacroError> {
  let name = item.ident.to_token_stream();
  let fields: Result<Vec<TokenStream>, MacroError> = if let Fields::Named(fields) = &s.fields {
    fields
      .named
      .iter()
      .map(|f| {
        editor_for_field(
          false,
          false,
          f.ident
            .clone()
            .ok_or(MacroError::Message("Field must have a name", f.span()))?,
          f,
        )
      })
      .collect()
  } else {
    Err(MacroError::Message(
      "EditableConfig only valid on named structs",
      item.span(),
    ))
  };

  let fields = fields?;

  Ok(quote! {
    impl crate::util::editable_config::EditableConfig for #name {
      fn edit(&mut self, ui: &crate::util::editable_config::EditableConfigUi) -> bool {
        let mut changed = false;
        #( #fields )*
        changed
      }
    }
  })
}

fn editable_config_impl(item: DeriveInput) -> Result<TokenStream, MacroError> {
  let visitor = match &item.data {
    syn::Data::Struct(s) => editor_for_struct(&item, s),
    syn::Data::Enum(s) => editor_for_enum(&item, s),
    _ => Err(MacroError::Message(
      "EditableConfig is only valid for structs and enums",
      item.span(),
    )),
  }?;

  Ok(visitor)
}

fn editor_for_enum(item: &DeriveInput, enum_val: &DataEnum) -> Result<TokenStream, MacroError> {
  let name = item.ident.to_token_stream();

  let item_names: Vec<String> = enum_val
    .variants
    .iter()
    .map(|v| v.ident.to_string())
    .collect();

  let match_arms: Vec<TokenStream> = enum_val
    .variants
    .iter()
    .map(|v| {
      let variant_ident = v.ident.clone();
      let variant_name = v.ident.to_string();
      match v.fields {
        Fields::Unit => quote! { #name::#variant_ident => #variant_name },
        Fields::Named(..) => quote! { #name::#variant_ident { .. } => #variant_name },
        Fields::Unnamed(..) => quote! { #name::#variant_ident(..) => #variant_name },
      }
    })
    .collect();

  let mut if_statements: Vec<TokenStream> = vec![];
  for v in &enum_val.variants {
    let variant_ident = v.ident.clone();
    let variant_name = v.ident.to_string();
    let body = match &v.fields {
      Fields::Named(fields_named) => {
        let fields: Result<Vec<TokenStream>, MacroError> = fields_named
          .named
          .iter()
          .map(|f| {
            editor_for_field(
              true,
              false,
              f.ident
                .clone()
                .ok_or(MacroError::Message("Field must have a name", f.span()))?,
              f,
            )
          })
          .collect();
        let fields = fields?;
        let field_idents: Vec<Ident> = fields_named
          .named
          .iter()
          .map(|f| f.ident.clone().unwrap())
          .collect();

        quote! {
          match &self {
            #name::#variant_ident { #(#field_idents,)* } => {
              #(#fields)*
              #name::#variant_ident { #(#field_idents,)* }
            }
          }
        }
      }
      Fields::Unnamed(fields_unnamed) => {
        if fields_unnamed.unnamed.len() != 1 {
          return Err(MacroError::Message(
            "Enum can only have one unnamed field",
            fields_unnamed.span(),
          ));
        }

        let ident = syn::Ident::new("val", fields_unnamed.span());
        let editor = editor_for_field(true, true, ident, &fields_unnamed.unnamed[0])?;

        quote! {
          match &self {
            #name::#variant_ident(val) => {
              let val = val.clone();
              #editor
              #name::#variant_ident(val)
            },
            _ => panic!("Unexpected current value")
          }
        }
      }
      Fields::Unit => quote! { #name::#variant_ident },
    };

    if !if_statements.is_empty() {
      if_statements.push(quote! { else });
    }

    if_statements.push(quote! {
      if new_name == #variant_name {
        #body
      }
    });
  }

  Ok(quote! {
    impl crate::util::editable_config::EditableConfig for #name {
      fn edit(&mut self, ui: &crate::util::editable_config::EditableConfigUi) -> bool {
        let mut changed = false;
        let names = vec![#( #item_names, )*];
        let current_name = match &self {
          #( #match_arms, )*
        };
        let new_name = (ui.dropdown)("", &mut changed, &names, current_name);
        *self = #( #if_statements )* else { panic!("Invalid enum value"); };
        changed
      }
    }
  })
}

/// #[derive(EditableConfig)]
///
/// possible attributes:
/// #[slider(min, max)]
/// #[float] or #[float(min)] or #[float(min, max)]
/// #[integer] or #[integer(min)] or #[integer(min, max)]
#[proc_macro_derive(EditableConfig, attributes(slider, float, integer))]
pub fn editable_config(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
  let item: DeriveInput = syn::parse(input).unwrap();
  match editable_config_impl(item) {
    Ok(stream) => stream.into(),
    Err(err) => err.to_token_stream().into(),
  }
}
