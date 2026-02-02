//! trader-strategy를 위한 프로시저 매크로.
//!
//! 이 크레이트는 전략 설정 구조체에 대한 SDUI 스키마 자동 생성을 제공합니다.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

/// StrategyConfig derive 매크로.
///
/// 전략 설정 구조체에 `ui_schema()` 메서드를 자동 생성합니다.
///
/// # Attributes
///
/// ## Container attributes (구조체)
/// - `#[strategy(id = "...", name = "...", description = "...", category = "...")]`
///   - `id`: 전략 ID (필수)
///   - `name`: 전략 이름 (필수)
///   - `description`: 전략 설명 (선택)
///   - `category`: 전략 카테고리 (필수)
///
/// ## Field attributes
/// - `#[fragment("fragment_id")]`: 이 필드가 Fragment를 사용함을 표시
/// - `#[fragment("fragment_id", optional)]`: 선택적 Fragment
/// - `#[schema(label = "...", min = ..., max = ...)]`: 커스텀 필드 메타데이터
///
/// # Examples
///
/// ```ignore
/// use trader_strategy_macro::StrategyConfig;
///
/// #[derive(StrategyConfig)]
/// #[strategy(
///     id = "rsi_mean_reversion",
///     name = "RSI 평균회귀",
///     description = "RSI 과매수/과매도 구간에서 평균회귀 매매",
///     category = "single_asset"
/// )]
/// pub struct RsiConfig {
///     #[fragment("indicator.rsi")]
///     pub rsi_period: i32,
///
///     #[schema(label = "쿨다운 캔들 수", min = 0, max = 100)]
///     pub cooldown_candles: usize,
/// }
/// ```
#[proc_macro_derive(StrategyConfig, attributes(strategy, fragment, schema))]
pub fn derive_strategy_config(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let struct_name = &input.ident;

    // 구조체 속성에서 전략 메타데이터 추출
    let strategy_attrs = parse_strategy_attributes(&input.attrs);

    let strategy_id = strategy_attrs
        .get("id")
        .expect("strategy(id = \"...\") attribute is required");
    let strategy_name = strategy_attrs
        .get("name")
        .expect("strategy(name = \"...\") attribute is required");
    let strategy_category = strategy_attrs
        .get("category")
        .expect("strategy(category = \"...\") attribute is required");
    let strategy_description = strategy_attrs.get("description");

    // 필드 분석
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("StrategyConfig can only be derived for structs with named fields"),
        },
        _ => panic!("StrategyConfig can only be derived for structs"),
    };

    // Fragment 참조 수집
    let mut fragment_refs = Vec::new();
    // 커스텀 필드 수집
    let mut custom_fields = Vec::new();

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();

        // fragment 속성 확인
        let has_fragment = field
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("fragment"));

        if has_fragment {
            // Fragment 속성 파싱
            for attr in &field.attrs {
                if attr.path().is_ident("fragment") {
                    let (fragment_id, optional) = parse_fragment_attribute(attr);
                    fragment_refs.push(quote! {
                        trader_core::FragmentRef {
                            id: #fragment_id.to_string(),
                            required: #optional == false,
                        }
                    });
                }
            }
        } else {
            // 커스텀 필드
            let schema_attrs = parse_schema_attributes(&field.attrs);
            let field_name_str = field_name.to_string();

            let default_label = field_name_str.clone();
            let label = schema_attrs.get("label").unwrap_or(&default_label);
            let description = schema_attrs.get("description");
            let min = schema_attrs.get("min");
            let max = schema_attrs.get("max");

            // 필드 타입 추론
            let field_type = infer_field_type(&field.ty);

            let description_expr = if let Some(desc) = description {
                quote! { Some(#desc.to_string()) }
            } else {
                quote! { None }
            };

            let min_expr = if let Some(min_val) = min {
                let min_f64: f64 = min_val.parse().unwrap_or(0.0);
                quote! { Some(#min_f64) }
            } else {
                quote! { None }
            };

            let max_expr = if let Some(max_val) = max {
                let max_f64: f64 = max_val.parse().unwrap_or(100.0);
                quote! { Some(#max_f64) }
            } else {
                quote! { None }
            };

            custom_fields.push(quote! {
                trader_core::FieldSchema {
                    name: #field_name_str.to_string(),
                    field_type: #field_type,
                    label: #label.to_string(),
                    description: #description_expr,
                    min: #min_expr,
                    max: #max_expr,
                    required: true,
                    ..Default::default()
                }
            });
        }
    }

    let description_expr = if let Some(desc) = strategy_description {
        quote! { Some(#desc.to_string()) }
    } else {
        quote! { None }
    };

    // 생성된 코드
    let expanded = quote! {
        impl #struct_name {
            /// 전략의 UI 스키마를 반환합니다.
            pub fn ui_schema() -> trader_core::StrategyUISchema {
                trader_core::StrategyUISchema {
                    id: #strategy_id.to_string(),
                    name: #strategy_name.to_string(),
                    description: #description_expr,
                    category: #strategy_category.to_string(),
                    fragments: vec![
                        #(#fragment_refs),*
                    ],
                    custom_fields: vec![
                        #(#custom_fields),*
                    ],
                    defaults: None,
                }
            }
        }
    };

    TokenStream::from(expanded)
}

/// 구조체의 strategy 속성을 파싱합니다.
fn parse_strategy_attributes(
    attrs: &[syn::Attribute],
) -> std::collections::HashMap<String, String> {
    let mut result = std::collections::HashMap::new();

    for attr in attrs {
        if attr.path().is_ident("strategy") {
            if let Ok(meta_list) = attr.meta.require_list() {
                for nested in meta_list.tokens.clone().into_iter() {
                    let nested_str = nested.to_string();
                    // "id = \"value\"" 형태 파싱
                    if let Some((key, value)) = nested_str.split_once('=') {
                        let key = key.trim();
                        let value = value.trim().trim_matches('"');
                        result.insert(key.to_string(), value.to_string());
                    }
                }
            }
        }
    }

    result
}

/// fragment 속성을 파싱합니다.
fn parse_fragment_attribute(attr: &syn::Attribute) -> (String, bool) {
    let mut fragment_id = String::new();
    let mut optional = false;

    if let Ok(meta_list) = attr.meta.require_list() {
        let tokens_str = meta_list.tokens.to_string();
        let parts: Vec<&str> = tokens_str.split(',').map(|s| s.trim()).collect();

        for part in parts {
            if part.starts_with('"') {
                fragment_id = part.trim_matches('"').to_string();
            } else if part == "optional" {
                optional = true;
            }
        }
    }

    (fragment_id, optional)
}

/// 필드의 schema 속성을 파싱합니다.
fn parse_schema_attributes(attrs: &[syn::Attribute]) -> std::collections::HashMap<String, String> {
    let mut result = std::collections::HashMap::new();

    for attr in attrs {
        if attr.path().is_ident("schema") {
            if let Ok(meta_list) = attr.meta.require_list() {
                for nested in meta_list.tokens.clone().into_iter() {
                    let nested_str = nested.to_string();
                    if let Some((key, value)) = nested_str.split_once('=') {
                        let key = key.trim();
                        let value = value.trim().trim_matches('"');
                        result.insert(key.to_string(), value.to_string());
                    }
                }
            }
        }
    }

    result
}

/// 필드 타입으로부터 FieldType을 추론합니다.
fn infer_field_type(ty: &syn::Type) -> proc_macro2::TokenStream {
    let type_str = quote!(#ty).to_string();

    if type_str.contains("i32") || type_str.contains("i64") || type_str.contains("usize") {
        quote! { trader_core::FieldType::Integer }
    } else if type_str.contains("f32") || type_str.contains("f64") || type_str.contains("Decimal") {
        quote! { trader_core::FieldType::Number }
    } else if type_str.contains("bool") {
        quote! { trader_core::FieldType::Boolean }
    } else if type_str.contains("String") {
        quote! { trader_core::FieldType::String }
    } else if type_str.contains("Vec") {
        quote! { trader_core::FieldType::Symbols }
    } else {
        // 기본값
        quote! { trader_core::FieldType::String }
    }
}
