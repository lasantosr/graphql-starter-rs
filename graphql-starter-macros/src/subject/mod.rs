use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

pub(crate) fn r#impl(input: DeriveInput) -> TokenStream {
    let crate_expr = quote!(graphql_starter);
    let ident = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    quote!(impl #impl_generics #crate_expr::auth::Subject for #ident #ty_generics #where_clause {})
}
