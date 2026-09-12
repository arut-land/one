//! The subset of Fluent every target format can carry, and the parser that
//! proves a message is inside it.
//!
//! Fluent is richer than any platform resource format. Rather than degrade a
//! message the target cannot express, the parser refuses it by name, which is
//! what ADR 0022 asks for: the string source is allowed to grow only in ways
//! that survive the trip to all four platforms.

use std::collections::BTreeMap;
use std::fmt;

use fluent_syntax::ast;
use fluent_syntax::parser;

/// Everything one locale contributes, keyed by message id.
pub struct Locale {
    pub tag: String,
    pub messages: BTreeMap<String, Message>,
}

/// A message is either one pattern or one plural selection over patterns.
pub enum Message {
    Simple(Pattern),
    /// Fluent plural categories, in CLDR order, each already carrying whatever
    /// text surrounded the selector.
    Plural {
        selector: Argument,
        variants: Vec<(Category, Pattern)>,
    },
}

/// A pattern is literal text with argument placeholders punched through it.
#[derive(Default)]
pub struct Pattern {
    pub parts: Vec<Part>,
}

pub enum Part {
    Text(String),
    Argument(Argument),
}

/// One Fluent variable. `numeric` is set by wrapping the variable in `NUMBER()`
/// at the call site, which is how the source says "this is an integer" so a
/// target can emit `%lld` rather than `%@`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Argument {
    pub name: String,
    pub numeric: bool,
}

/// The CLDR plural categories Fluent, Apple and Android all share.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

impl Category {
    const ORDER: [Self; 6] = [
        Self::Zero,
        Self::One,
        Self::Two,
        Self::Few,
        Self::Many,
        Self::Other,
    ];

    fn parse(name: &str) -> Option<Self> {
        match name {
            "zero" => Some(Self::Zero),
            "one" => Some(Self::One),
            "two" => Some(Self::Two),
            "few" => Some(Self::Few),
            "many" => Some(Self::Many),
            "other" => Some(Self::Other),
            _ => None,
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::One => "one",
            Self::Two => "two",
            Self::Few => "few",
            Self::Many => "many",
            Self::Other => "other",
        }
    }
}

/// Why a message cannot be generated. Every one names the message, because the
/// person reading it is looking at a `.ftl` file, not at this code.
pub struct Refusal {
    pub locale: String,
    pub file: String,
    pub message: String,
    pub reason: String,
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}/{}: `{}` cannot be generated: {}",
            self.locale, self.file, self.message, self.reason
        )
    }
}

impl Message {
    /// Every argument the message uses, in first-appearance order.
    ///
    /// The order is the placeholder index on Apple, Android and Windows alike,
    /// and it is shared across a plural's variants so `%1$d` means the same
    /// thing in every one of them.
    #[must_use]
    pub fn arguments(&self) -> Vec<Argument> {
        let mut order: Vec<Argument> = Vec::new();
        let mut push = |argument: &Argument| {
            if !order.iter().any(|seen| seen.name == argument.name) {
                order.push(argument.clone());
            }
        };
        match self {
            Self::Simple(pattern) => pattern.arguments().for_each(&mut push),
            Self::Plural { selector, variants } => {
                push(selector);
                for (_, pattern) in variants {
                    pattern.arguments().for_each(&mut push);
                }
            }
        }
        order
    }
}

impl Pattern {
    fn arguments(&self) -> impl Iterator<Item = &Argument> {
        self.parts.iter().filter_map(|part| match part {
            Part::Argument(argument) => Some(argument),
            Part::Text(_) => None,
        })
    }
}

/// Parse every `.ftl` file of one locale into the generatable subset.
///
/// # Errors
///
/// Returns every refusal at once, so one run names all the work rather than one
/// message per run.
pub fn parse(tag: &str, resources: &[(&str, &str)]) -> Result<Locale, Vec<Refusal>> {
    let mut messages = BTreeMap::new();
    let mut refusals = Vec::new();
    for (file, source) in resources {
        let resource = match parser::parse(*source) {
            Ok(resource) => resource,
            Err((resource, errors)) => {
                refusals.push(Refusal {
                    locale: tag.to_owned(),
                    file: (*file).to_owned(),
                    message: "<file>".to_owned(),
                    reason: format!("the file is not valid Fluent: {errors:?}"),
                });
                resource
            }
        };
        for entry in resource.body {
            let refuse = |message: &str, reason: String| Refusal {
                locale: tag.to_owned(),
                file: (*file).to_owned(),
                message: message.to_owned(),
                reason,
            };
            match entry {
                ast::Entry::Message(message) => {
                    let id = message.id.name.to_owned();
                    if !message.attributes.is_empty() {
                        refusals.push(refuse(
                            &id,
                            "it has attributes, which no target resource format has".to_owned(),
                        ));
                        continue;
                    }
                    let Some(value) = message.value else {
                        refusals.push(refuse(&id, "it has no value".to_owned()));
                        continue;
                    };
                    if messages.contains_key(&id) {
                        refusals.push(refuse(&id, "it is defined twice in this locale".to_owned()));
                        continue;
                    }
                    match lower(&value) {
                        Ok(lowered) => {
                            messages.insert(id, lowered);
                        }
                        Err(reason) => refusals.push(refuse(&id, reason)),
                    }
                }
                ast::Entry::Term(term) if !term.attributes.is_empty() => {
                    refusals.push(refuse(
                        &format!("-{}", term.id.name),
                        "it is a term with attributes, which cannot be inlined into a resource string"
                            .to_owned(),
                    ));
                }
                // A term with no attributes defines nothing on its own; a
                // pattern that references one is refused where it is used.
                ast::Entry::Term(_)
                | ast::Entry::Comment(_)
                | ast::Entry::GroupComment(_)
                | ast::Entry::ResourceComment(_) => {}
                ast::Entry::Junk { content } => {
                    refusals.push(refuse(
                        "<junk>",
                        format!("the file has unparsable content: {content:?}"),
                    ));
                }
            }
        }
    }
    if refusals.is_empty() {
        Ok(Locale {
            tag: tag.to_owned(),
            messages,
        })
    } else {
        Err(refusals)
    }
}

/// Turn one Fluent pattern into a [`Message`], folding a single selector into a
/// plural and refusing everything a target cannot carry.
fn lower(pattern: &ast::Pattern<&str>) -> Result<Message, String> {
    let mut prefix = Pattern::default();
    let mut suffix = Pattern::default();
    let mut selection: Option<(Argument, Vec<(Category, Pattern)>)> = None;
    for element in &pattern.elements {
        match element {
            ast::PatternElement::TextElement { value } => {
                let target = if selection.is_some() {
                    &mut suffix
                } else {
                    &mut prefix
                };
                target.parts.push(Part::Text((*value).to_owned()));
            }
            ast::PatternElement::Placeable { expression } => match expression {
                ast::Expression::Select { selector, variants } => {
                    if selection.is_some() {
                        return Err("it has two selectors; a target resource can carry one plural selection per string".to_owned());
                    }
                    selection = Some(lower_selection(selector, variants)?);
                }
                ast::Expression::Inline(inline) => {
                    let part = Part::Argument(lower_argument(inline)?);
                    let target = if selection.is_some() {
                        &mut suffix
                    } else {
                        &mut prefix
                    };
                    target.parts.push(part);
                }
            },
        }
    }
    let Some((selector, variants)) = selection else {
        return Ok(Message::Simple(prefix));
    };
    // Text on either side of the selector is duplicated into every variant,
    // because that is the only shape `<plurals>` and an xcstrings variation
    // can hold.
    let variants = variants
        .into_iter()
        .map(|(category, body)| {
            let mut parts = Vec::new();
            parts.extend(clone_parts(&prefix));
            parts.extend(body.parts);
            parts.extend(clone_parts(&suffix));
            (category, Pattern { parts })
        })
        .collect();
    Ok(Message::Plural { selector, variants })
}

fn clone_parts(pattern: &Pattern) -> Vec<Part> {
    pattern
        .parts
        .iter()
        .map(|part| match part {
            Part::Text(text) => Part::Text(text.clone()),
            Part::Argument(argument) => Part::Argument(argument.clone()),
        })
        .collect()
}

fn lower_selection(
    selector: &ast::InlineExpression<&str>,
    variants: &[ast::Variant<&str>],
) -> Result<(Argument, Vec<(Category, Pattern)>), String> {
    let selector = lower_argument(selector)?;
    let mut by_category: BTreeMap<&'static str, Pattern> = BTreeMap::new();
    for variant in variants {
        let name = match &variant.key {
            ast::VariantKey::Identifier { name } => *name,
            ast::VariantKey::NumberLiteral { .. } => {
                return Err("it selects on an exact number; only the CLDR plural categories (zero, one, two, few, many, other) reach a native plural resource".to_owned());
            }
        };
        let Some(category) = Category::parse(name) else {
            return Err(format!(
                "it has a `{name}` variant, which is not a CLDR plural category"
            ));
        };
        let mut body = Pattern::default();
        for element in &variant.value.elements {
            match element {
                ast::PatternElement::TextElement { value } => {
                    body.parts.push(Part::Text((*value).to_owned()));
                }
                ast::PatternElement::Placeable { expression } => match expression {
                    ast::Expression::Select { .. } => {
                        return Err("it nests a selector inside a variant, which no target resource format can express".to_owned());
                    }
                    ast::Expression::Inline(inline) => {
                        body.parts.push(Part::Argument(lower_argument(inline)?));
                    }
                },
            }
        }
        by_category.insert(category.name(), body);
    }
    if !by_category.contains_key("other") {
        return Err("it has no `other` variant, which every plural resource requires".to_owned());
    }
    let ordered = Category::ORDER
        .into_iter()
        .filter_map(|category| {
            by_category
                .remove(category.name())
                .map(|pattern| (category, pattern))
        })
        .collect();
    Ok((selector, ordered))
}

fn lower_argument(inline: &ast::InlineExpression<&str>) -> Result<Argument, String> {
    match inline {
        ast::InlineExpression::VariableReference { id } => Ok(Argument {
            name: (*id.name).to_owned(),
            numeric: false,
        }),
        ast::InlineExpression::FunctionReference { id, arguments } => {
            if id.name != "NUMBER" {
                return Err(format!(
                    "it calls `{}()`; only NUMBER() survives into a native resource string",
                    id.name
                ));
            }
            if !arguments.named.is_empty() {
                return Err(
                    "it passes options to NUMBER(); a native placeholder carries no formatting options"
                        .to_owned(),
                );
            }
            match arguments.positional.as_slice() {
                [ast::InlineExpression::VariableReference { id }] => Ok(Argument {
                    name: (*id.name).to_owned(),
                    numeric: true,
                }),
                _ => Err("it calls NUMBER() on something other than one variable".to_owned()),
            }
        }
        ast::InlineExpression::TermReference { id, .. } => Err(format!(
            "it references the term `-{}`; terms are not carried into resource files",
            id.name
        )),
        ast::InlineExpression::MessageReference { id, .. } => Err(format!(
            "it references the message `{}`; a resource string cannot compose another",
            id.name
        )),
        ast::InlineExpression::Placeable { .. } => {
            Err("it nests a placeable inside a placeable".to_owned())
        }
        ast::InlineExpression::StringLiteral { .. }
        | ast::InlineExpression::NumberLiteral { .. } => {
            Err("it places a literal, which belongs in the text itself".to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Category, Message, Part, parse};

    fn refusal(source: &str) -> String {
        let refusals = parse("en", &[("test.ftl", source)]).err().expect("refused");
        refusals
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn only(source: &str) -> Message {
        let locale = parse("en", &[("test.ftl", source)]).ok().expect("parsed");
        locale.messages.into_values().next().expect("one message")
    }

    #[test]
    fn text_and_variables_become_parts_in_order() {
        let Message::Simple(pattern) = only("k = a { $one } b { NUMBER($two) }") else {
            panic!("expected a simple message");
        };
        assert_eq!(pattern.parts.len(), 4);
        assert!(matches!(&pattern.parts[0], Part::Text(text) if text == "a "));
        let arguments = Message::Simple(pattern).arguments();
        assert_eq!(arguments[0].name, "one");
        assert!(!arguments[0].numeric);
        assert_eq!(arguments[1].name, "two");
        assert!(arguments[1].numeric);
    }

    #[test]
    fn a_selector_becomes_a_plural_with_the_surrounding_text_in_every_variant() {
        let message = only(
            "k = You have { $count -> \n    [one] one message\n   *[other] { $count } messages\n } waiting.",
        );
        let Message::Plural { selector, variants } = &message else {
            panic!("expected a plural");
        };
        assert_eq!(selector.name, "count");
        assert_eq!(variants.len(), 2);
        assert_eq!(variants[0].0, Category::One);
        assert_eq!(variants[1].0, Category::Other);
        for (_, pattern) in variants {
            assert!(matches!(&pattern.parts[0], Part::Text(text) if text == "You have "));
            assert!(matches!(pattern.parts.last(), Some(Part::Text(text)) if text == " waiting."));
        }
        // The selector counts as the first argument even where a variant does
        // not interpolate it, so `%1$d` means the same in both.
        assert_eq!(message.arguments().len(), 1);
    }

    #[test]
    fn a_nested_selector_is_refused_by_name() {
        let refused =
            refusal("outer = { $a ->\n   *[other] { $b ->\n       *[other] x\n     }\n }");
        assert!(refused.contains("`outer`"), "{refused}");
        assert!(refused.contains("nests a selector"), "{refused}");
    }

    #[test]
    fn a_term_with_attributes_is_refused_by_name() {
        let refused = refusal("-brand = Arut\n    .gender = neuter\n");
        assert!(refused.contains("`-brand`"), "{refused}");
        assert!(refused.contains("term with attributes"), "{refused}");
    }

    #[test]
    fn a_term_reference_is_refused_by_the_message_that_uses_it() {
        let refused = refusal("-brand = Arut\nk = Welcome to { -brand }.");
        assert!(refused.contains("`k`"), "{refused}");
        assert!(refused.contains("-brand"), "{refused}");
    }

    #[test]
    fn message_attributes_and_message_references_are_refused() {
        assert!(refusal("k = v\n    .tooltip = t\n").contains("attributes"));
        assert!(refusal("a = x\nb = { a }").contains("cannot compose another"));
    }

    #[test]
    fn an_unknown_function_and_number_options_are_refused() {
        assert!(refusal("k = { DATETIME($when) }").contains("DATETIME()"));
        assert!(
            refusal("k = { NUMBER($n, minimumFractionDigits: 2) }").contains("options to NUMBER()")
        );
    }

    #[test]
    fn a_plural_without_an_other_variant_and_an_exact_number_key_are_refused() {
        assert!(refusal("k = { $n ->\n   *[0] none\n }").contains("exact number"));
        assert!(refusal("k = { $n ->\n   *[weird] w\n }").contains("not a CLDR plural category"));
    }

    #[test]
    fn two_selectors_in_one_message_are_refused() {
        let refused = refusal("k = { $a ->\n   *[other] x\n } and { $b ->\n   *[other] y\n }");
        assert!(refused.contains("two selectors"), "{refused}");
    }
}
