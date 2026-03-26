use super::{Forall, GenericName, Intsize, KnownType};
use std::collections::HashMap;

/// For illustrative purposes we use unrealistic static strings
pub type Name = &'static str;
pub type Field = &'static str;

#[derive(Debug, Clone)]
pub enum Error {
    RecordNotFound(Name),
    FieldNotFound(Field),
}

/// For illustrative purposes we oversimplify name resolution
pub fn type_of_field(name: Name, field: Field) -> Result<KnownType, Error> {
    match name {
        "Pair" => match field {
            "first" => Ok(KnownType::Generic("a")),
            "second" => Ok(KnownType::Generic("b")),
            _ => Err(Error::FieldNotFound(field)),
        },
        "Point" => match field {
            "x" => Ok(KnownType::Generic("a")),
            "y" => Ok(KnownType::Generic("a")),
            _ => Err(Error::FieldNotFound(field)),
        },
        "Labeled" => match field {
            "id" => Ok(KnownType::Int(Intsize::default())),
            "label" => Ok(KnownType::String),
            "value" => Ok(KnownType::Generic("a")),
            _ => Err(Error::FieldNotFound(field)),
        },
        _ => Err(Error::RecordNotFound(name)),
    }
}

/// For illustrative purposes we oversimplify name resolution
pub fn fields_of(name: Name) -> Option<&'static [Field]> {
    match name {
        "Pair" => Some(&["first", "second"]),
        "Point" => Some(&["x", "y"]),
        "Labeled" => Some(&["id", "label", "value"]),
        _ => None,
    }
}

/// For illustrative purposes we oversimplify name resolution
pub fn guess_by_fields<I>(fields: I) -> Option<Name>
where
    I: Iterator<Item = Field> + Clone,
{
    let fields = fields.into_iter();

    ["Pair", "Point", "Labeled"].into_iter().find(|record| {
        fields_of(record)
            .iter()
            .any(|record_fields| fields.clone().any(|field| record_fields.contains(&field)))
    })
}

/// Map the type parameters of a record declaration
pub fn type_parameters<T>(
    name: Name,
    mut f: impl FnMut(GenericName) -> T,
) -> Result<Forall<T>, Error> {
    let generics = match name {
        "Pair" => ["a", "b"].as_slice(),
        "Point" => ["a"].as_slice(),
        "Labeled" => ["a"].as_slice(),
        _ => return Err(Error::RecordNotFound(name)),
    };

    Ok(generics.iter().map(|&name| (name, f(name))).collect())
}
