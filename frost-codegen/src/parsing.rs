use chumsky::prelude::*;

#[derive(Debug)]
struct Type {
    package_name: Option<String>,
    name: String,
}

#[derive(Debug)]
pub(crate) enum Statement {
    Field {
        msg_type: Type,
        name: String,
    },
    Constant {
        msg_type: Type,
        name: String,
        value: String,
    },
}

fn parser() -> impl Parser<char, Vec<Statement>, Error = Simple<char>> {
    let comment = just("#").then(take_until(just('\n'))).padded();

    let package = text::ident()
        .then_ignore(just("/"))
        .labelled("package")
        .or_not();

    let type_name = package
        .then(text::ident())
        .padded()
        .labelled("msg type")
        .map(|(package, type_name)| Type {
            package_name: package,
            name: type_name,
        });

    let name = type_name
        .then(text::ident())
        .then_ignore(just('='))
        .then(take_until(just('\n')))
        .padded()
        .map(|((msg_type, name), value)| Statement::Constant {
            msg_type,
            name,
            value: value.0.into_iter().collect(),
        })
        .or(type_name
            .then(text::ident())
            .padded()
            .map(|(msg_type, value)| Statement::Field {
                msg_type,
                name: value,
            }));

    name.padded_by(comment.repeated().or_not()).repeated()
}