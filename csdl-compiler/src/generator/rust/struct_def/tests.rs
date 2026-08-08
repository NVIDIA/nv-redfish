use super::StructDef;
use crate::compiler::RigidArraySupport;
use crate::IsNullable;
use crate::IsRequired;
use crate::OneOrCollection;

use proc_macro2::Literal;
use proc_macro2::TokenStream;
use quote::quote;

// A case of the field-generation matrix that affects the coordinated
// serde annotation and Rust field type generation. Shared by the
// action-parameter and read-struct tests: the inputs and expectations
// have the same shape, only the generator under test differs.
//
// Note that a deprecated property enters the read-struct matrix with
// required=false regardless of `Redfish.Required` (see
// `RedfishProperty::new`), so the "optional" cases are the shape of
// e.g. `Event.EventRecord/EventType`.
struct FieldCase {
    name: &'static str,
    cardinality: OneOrCollection<()>,
    nullable: bool,
    required: bool,
    expected_serde_annotation: TokenStream,
    expected_field_type: TokenStream,
}

fn check_field_cases(
    cases: impl IntoIterator<Item = FieldCase>,
    generate: impl Fn(&FieldCase) -> (TokenStream, TokenStream),
) {
    for case in cases {
        let (serde_annotation, field_type) = generate(&case);
        assert_token_eq(
            &serde_annotation,
            &case.expected_serde_annotation,
            case.name,
            "serde annotation",
        );
        assert_token_eq(
            &field_type,
            &case.expected_field_type,
            case.name,
            "field type",
        );
    }
}

fn gen_action_parameter(case: &FieldCase) -> (TokenStream, TokenStream) {
    let field = StructDef::gen_action_parameter_field(
        &case.cardinality,
        quote! { TestType },
        Literal::string("TestParam"),
        IsNullable::new(case.nullable),
        IsRequired::new(case.required),
    );
    (field.serde_annotation, field.field_type)
}

fn gen_de_struct_field(case: &FieldCase) -> (TokenStream, TokenStream) {
    StructDef::gen_de_struct_field(
        &case.cardinality,
        quote! { TestType },
        Literal::string("TestProp"),
        IsNullable::new(case.nullable),
        IsRequired::new(case.required),
        RigidArraySupport::new(false),
    )
}

fn assert_token_eq(actual: &TokenStream, expected: &TokenStream, case: &str, field: &str) {
    assert_eq!(actual.to_string(), expected.to_string(), "{case}: {field}");
}

#[test]
fn action_parameter_field_generation_scalar_combinations() {
    check_field_cases(
        [
            FieldCase {
                name: "required scalar",
                cardinality: OneOrCollection::One(()),
                nullable: false,
                required: true,
                expected_serde_annotation: quote! { #[serde(rename = "TestParam")] },
                expected_field_type: quote! { TestType },
            },
            FieldCase {
                name: "required nullable scalar",
                cardinality: OneOrCollection::One(()),
                nullable: true,
                required: true,
                expected_serde_annotation: quote! { #[serde(rename = "TestParam")] },
                expected_field_type: quote! { Option<TestType> },
            },
            FieldCase {
                name: "optional scalar",
                cardinality: OneOrCollection::One(()),
                nullable: false,
                required: false,
                expected_serde_annotation: quote! {
                    #[serde(rename = "TestParam", skip_serializing_if = "Option::is_none")]
                },
                expected_field_type: quote! { Option<TestType> },
            },
            FieldCase {
                name: "optional nullable scalar",
                cardinality: OneOrCollection::One(()),
                nullable: true,
                required: false,
                expected_serde_annotation: quote! {
                    #[serde(rename = "TestParam", skip_serializing_if = "Option::is_none")]
                },
                expected_field_type: quote! { Option<Option<TestType>> },
            },
        ],
        gen_action_parameter,
    );
}

#[test]
fn action_parameter_field_generation_collection_combinations() {
    check_field_cases(
        [
            FieldCase {
                name: "required collection",
                cardinality: OneOrCollection::Collection(()),
                nullable: false,
                required: true,
                expected_serde_annotation: quote! { #[serde(rename = "TestParam")] },
                expected_field_type: quote! { Vec<TestType> },
            },
            FieldCase {
                name: "required nullable collection",
                cardinality: OneOrCollection::Collection(()),
                nullable: true,
                required: true,
                expected_serde_annotation: quote! { #[serde(rename = "TestParam")] },
                expected_field_type: quote! { Option<Vec<TestType>> },
            },
            FieldCase {
                name: "optional collection",
                cardinality: OneOrCollection::Collection(()),
                nullable: false,
                required: false,
                expected_serde_annotation: quote! {
                    #[serde(rename = "TestParam", skip_serializing_if = "Option::is_none")]
                },
                expected_field_type: quote! { Option<Vec<TestType>> },
            },
            FieldCase {
                name: "optional nullable collection",
                cardinality: OneOrCollection::Collection(()),
                nullable: true,
                required: false,
                expected_serde_annotation: quote! {
                    #[serde(rename = "TestParam", skip_serializing_if = "Option::is_none")]
                },
                expected_field_type: quote! { Option<Option<Vec<TestType>>> },
            },
        ],
        gen_action_parameter,
    );
}

#[test]
fn de_struct_field_generation_scalar_combinations() {
    check_field_cases(
        [
            FieldCase {
                name: "required scalar",
                cardinality: OneOrCollection::One(()),
                nullable: false,
                required: true,
                expected_serde_annotation: quote! { #[serde(rename = "TestProp")] },
                expected_field_type: quote! { TestType },
            },
            FieldCase {
                name: "required nullable scalar",
                cardinality: OneOrCollection::One(()),
                nullable: true,
                required: true,
                expected_serde_annotation: quote! {
                    #[serde(rename = "TestProp", deserialize_with = "de_required_nullable")]
                },
                expected_field_type: quote! { Option<TestType> },
            },
            FieldCase {
                name: "optional scalar",
                cardinality: OneOrCollection::One(()),
                nullable: false,
                required: false,
                expected_serde_annotation: quote! { #[serde(rename = "TestProp", default)] },
                expected_field_type: quote! { Option<TestType> },
            },
            FieldCase {
                name: "optional nullable scalar",
                cardinality: OneOrCollection::One(()),
                nullable: true,
                required: false,
                expected_serde_annotation: quote! {
                    #[serde(rename = "TestProp", default, deserialize_with = "de_optional_nullable")]
                },
                expected_field_type: quote! { Option<Option<TestType>> },
            },
        ],
        gen_de_struct_field,
    );
}

#[test]
fn de_struct_field_generation_collection_combinations() {
    check_field_cases(
        [
            FieldCase {
                name: "required collection",
                cardinality: OneOrCollection::Collection(()),
                nullable: false,
                required: true,
                expected_serde_annotation: quote! { #[serde(rename = "TestProp")] },
                expected_field_type: quote! { Vec<TestType> },
            },
            FieldCase {
                name: "required nullable collection",
                cardinality: OneOrCollection::Collection(()),
                nullable: true,
                required: true,
                expected_serde_annotation: quote! {
                    #[serde(rename = "TestProp", deserialize_with = "de_required_nullable")]
                },
                expected_field_type: quote! { Option<Vec<TestType>> },
            },
            FieldCase {
                name: "optional collection",
                cardinality: OneOrCollection::Collection(()),
                nullable: false,
                required: false,
                expected_serde_annotation: quote! { #[serde(rename = "TestProp", default)] },
                expected_field_type: quote! { Option<Vec<TestType>> },
            },
            FieldCase {
                name: "optional nullable collection",
                cardinality: OneOrCollection::Collection(()),
                nullable: true,
                required: false,
                expected_serde_annotation: quote! {
                    #[serde(rename = "TestProp", default, deserialize_with = "de_optional_nullable")]
                },
                expected_field_type: quote! { Option<Option<Vec<TestType>>> },
            },
        ],
        gen_de_struct_field,
    );
}
