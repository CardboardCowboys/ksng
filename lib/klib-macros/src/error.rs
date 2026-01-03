use proc_macro2::{LexError, Span, TokenStream};
use quote::{ToTokens, TokenStreamExt, quote};

#[derive(Debug)]
pub enum MacroError {
  Message(&'static str, Span),
  LexerError(String, Span),
  Syn(syn::Error),
}

impl From<LexError> for MacroError {
  fn from(value: LexError) -> Self {
    MacroError::LexerError(
      format!("LexError encountered while building macro: {}", value),
      value.span(),
    )
  }
}

impl From<syn::Error> for MacroError {
  fn from(value: syn::Error) -> Self {
    MacroError::Syn(value)
  }
}

fn str_to_tokens(str: &impl ToString) -> TokenStream {
  let owned = str.to_string();
  quote! { compile_error!(#owned); }
}

fn str_and_loc_to_tokens<T: ToString + ?Sized>(str: &T, span: &Span) -> TokenStream {
  let msg = format!("error at callsite {:?}: {}", span, str.to_string());
  str_to_tokens(&msg)
}

impl ToTokens for MacroError {
  #[track_caller]
  fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
    let new_tokens = match self {
      Self::Message(str, loc) => str_and_loc_to_tokens(str, loc),
      Self::LexerError(str, loc) => str_and_loc_to_tokens(str, loc),
      Self::Syn(syn) => syn.to_compile_error(),
    };

    tokens.append_all(new_tokens)
  }
}
