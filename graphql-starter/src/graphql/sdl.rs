use std::{fs, io, path::Path};

use async_graphql::{ObjectType, SDLExportOptions, Schema, SubscriptionType};

// Exports the GraphQL SDL to the provided path
pub fn export_graphql_sdl<Query, Mutation, Subscription>(
    schema: &Schema<Query, Mutation, Subscription>,
    path: impl AsRef<Path>,
    federation: bool,
) -> io::Result<()>
where
    Query: ObjectType + 'static,
    Mutation: ObjectType + 'static,
    Subscription: SubscriptionType + 'static,
{
    if federation {
        fs::write(path, schema.sdl_with_options(SDLExportOptions::new().federation()))?;
    } else {
        fs::write(path, schema.sdl())?;
    }

    Ok(())
}
