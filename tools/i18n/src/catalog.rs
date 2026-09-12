//! The subset of Fluent every target format can carry, and the parser that
//! proves a message is inside it.
//!
//! Fluent is richer than any platform resource format. Rather than degrade a
//! message the target cannot express, the parser refuses it by name, which is
//! what ADR 0022 asks for: the string source is allowed to grow only in ways
//! that survive the trip to all four platforms.
//!
//! Arguments are typed here too, because every generated accessor needs a
//! parameter type. A variable a plural selects on, or one wrapped in
//! `NUMBER()`, is a number; one wrapped in `DATETIME()` is a date; anything
//! else is text. A comment line on the message overrides that:
//! `# $current: integer`.

use std::collections::{BTreeMap, BTreeSet};
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
#[derive(Clone, Default)]
pub struct Pattern {
    pub parts: Vec<Part>,
}

#[derive(Clone)]
pub enum Part {
    Text(String),
    Argument(Argument),
}

/// What a generated accessor takes for one Fluent variable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// An integer: `u64`, `Int`, `Long`, `long`, `number`.
    Number,
    /// A date. It reaches Fluent already formatted, because `fluent-rs` has no
    /// datetime value to hand a bundle, so the carrier type is text on every
    /// platform; the kind exists so `DATETIME()` is recognized rather than
    /// refused, and so a translator sees what the argument means.
    Date,
    /// Anything else.
    Text,
}

/// One Fluent variable and the type its accessor takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Argument {
    pub name: String,
    pub kind: Kind,
}

impl Argument {
    /// Whether a target should emit its integer placeholder for this argument.
    #[must_use]
    pub fn numeric(&self) -> bool {
        self.kind == Kind::Number
    }
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
    /// the parameter order of every generated accessor, and it is shared across
    /// a plural's variants so `%1$d` means the same thing in every one of them.
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

    /// The argument a plural selects on, if this message is one.
    #[must_use]
    pub fn selector(&self) -> Option<&Argument> {
        match self {
            Self::Simple(_) => None,
            Self::Plural { selector, .. } => Some(selector),
        }
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
                    let overrides = match kind_overrides(message.comment.as_ref()) {
                        Ok(overrides) => overrides,
                        Err(reason) => {
                            refusals.push(refuse(&id, reason));
                            continue;
                        }
                    };
                    match lower(&value, &overrides) {
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

/// Every locale has to define exactly the same ids, or a generated accessor
/// would compile against a string one language does not have.
///
/// # Errors
///
/// Names the locale and the ids it is missing or holds alone.
pub fn require_identical_key_sets(locales: &[Locale]) -> Result<(), String> {
    let Some(first) = locales.first() else {
        return Ok(());
    };
    let expected: BTreeSet<&String> = first.messages.keys().collect();
    let mut problems = Vec::new();
    for locale in &locales[1..] {
        let ids: BTreeSet<&String> = locale.messages.keys().collect();
        let missing: Vec<&&String> = expected.difference(&ids).collect();
        let extra: Vec<&&String> = ids.difference(&expected).collect();
        if !missing.is_empty() {
            problems.push(format!(
                "locale {} is missing {:?}, which {} defines",
                locale.tag, missing, first.tag
            ));
        }
        if !extra.is_empty() {
            problems.push(format!(
                "locale {} defines {:?}, which {} does not",
                locale.tag, extra, first.tag
            ));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

/// Read `# $name: integer` lines off a message's comment.
fn kind_overrides(comment: Option<&ast::Comment<&str>>) -> Result<BTreeMap<String, Kind>, String> {
    let mut overrides = BTreeMap::new();
    let Some(comment) = comment else {
        return Ok(overrides);
    };
    for line in &comment.content {
        let Some(rest) = line.trim().strip_prefix('$') else {
            continue;
        };
        let Some((name, kind)) = rest.split_once(':') else {
            continue;
        };
        let kind = match kind.trim() {
            "integer" | "number" => Kind::Number,
            "date" => Kind::Date,
            "string" | "text" => Kind::Text,
            other => {
                return Err(format!(
                    "its comment types `${}` as `{other}`; use integer, number, date, string or text",
                    name.trim()
                ));
            }
        };
        overrides.insert(name.trim().to_owned(), kind);
    }
    Ok(overrides)
}

/// Turn one Fluent pattern into a [`Message`], folding a single selector into a
/// plural and refusing everything a target cannot carry.
fn lower(
    pattern: &ast::Pattern<&str>,
    overrides: &BTreeMap<String, Kind>,
) -> Result<Message, String> {
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
                    selection = Some(lower_selection(selector, variants, overrides)?);
                }
                ast::Expression::Inline(inline) => {
                    let part = Part::Argument(lower_argument(inline, overrides)?);
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
            parts.extend(prefix.parts.iter().cloned());
            parts.extend(body.parts);
            parts.extend(suffix.parts.iter().cloned());
            (category, Pattern { parts })
        })
        .collect();
    Ok(Message::Plural { selector, variants })
}

fn lower_selection(
    selector: &ast::InlineExpression<&str>,
    variants: &[ast::Variant<&str>],
    overrides: &BTreeMap<String, Kind>,
) -> Result<(Argument, Vec<(Category, Pattern)>), String> {
    let mut selector = lower_argument(selector, overrides)?;
    // A plural selects on a count, whatever the source wrote around it.
    if !overrides.contains_key(&selector.name) {
        selector.kind = Kind::Number;
    }
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
                        let mut argument = lower_argument(inline, overrides)?;
                        if argument.name == selector.name {
                            argument.kind = selector.kind;
                        }
                        body.parts.push(Part::Argument(argument));
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

fn lower_argument(
    inline: &ast::InlineExpression<&str>,
    overrides: &BTreeMap<String, Kind>,
) -> Result<Argument, String> {
    let typed = |name: &str, inferred: Kind| Argument {
        name: name.to_owned(),
        kind: overrides.get(name).copied().unwrap_or(inferred),
    };
    match inline {
        ast::InlineExpression::VariableReference { id } => Ok(typed(id.name, Kind::Text)),
        ast::InlineExpression::FunctionReference { id, arguments } => {
            let inferred = match id.name {
                "NUMBER" => Kind::Number,
                "DATETIME" => Kind::Date,
                other => {
                    return Err(format!(
                        "it calls `{other}()`; only NUMBER() and DATETIME() survive into a native resource string"
                    ));
                }
            };
            if !arguments.named.is_empty() {
                return Err(format!(
                    "it passes options to {}(); a native placeholder carries no formatting options",
                    id.name
                ));
            }
            match arguments.positional.as_slice() {
                [ast::InlineExpression::VariableReference { id: variable }] => {
                    Ok(typed(variable.name, inferred))
                }
                _ => Err(format!(
                    "it calls {}() on something other than one variable",
                    id.name
                )),
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
pub(crate) mod tests {
    use super::{Category, Kind, Locale, Message, Part, parse, require_identical_key_sets};

    /// One locale from one inline source; the emitter tests build theirs here.
    pub(crate) fn locale(tag: &str, source: &str) -> Locale {
        parse(tag, &[("test.ftl", source)]).ok().expect("parsed")
    }

    fn refusal(source: &str) -> String {
        let refusals = parse("en", &[("test.ftl", source)]).err().expect("refused");
        refusals
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn only(source: &str) -> Message {
        locale("en", source)
            .messages
            .into_values()
            .next()
            .expect("one message")
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
        assert_eq!(arguments[0].kind, Kind::Text);
        assert_eq!(arguments[1].name, "two");
        assert_eq!(arguments[1].kind, Kind::Number);
    }

    #[test]
    fn datetime_types_an_argument_as_a_date_rather_than_being_refused() {
        assert_eq!(
            only("k = last seen { DATETIME($when) }").arguments()[0].kind,
            Kind::Date
        );
    }

    #[test]
    fn a_comment_overrides_the_inferred_type() {
        assert_eq!(
            only("# $count: integer\nk = { $count } left").arguments()[0].kind,
            Kind::Number
        );
        assert!(refusal("# $count: colour\nk = { $count }").contains("use integer"));
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
        // Selecting on a variable is what makes it a number, with no NUMBER().
        assert_eq!(selector.kind, Kind::Number);
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
        assert_eq!(message.arguments()[0].kind, Kind::Number);
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
        assert!(refusal("k = { CURRENCY($amount) }").contains("CURRENCY()"));
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

    #[test]
    fn locales_that_do_not_define_the_same_ids_are_refused_by_name() {
        let same = [
            locale("en", "a = x\nb = y\n"),
            locale("fr", "a = x\nb = y\n"),
        ];
        assert!(require_identical_key_sets(&same).is_ok());
        let short = [locale("en", "a = x\nb = y\n"), locale("fr", "a = x\n")];
        let error = require_identical_key_sets(&short).expect_err("refused");
        assert!(error.contains("fr"), "{error}");
        assert!(error.contains('b'), "{error}");
        let long = [locale("en", "a = x\n"), locale("fr", "a = x\nc = z\n")];
        assert!(
            require_identical_key_sets(&long)
                .expect_err("refused")
                .contains('c')
        );
    }
}
