//! SDUI 스키마 조합기.
//!
//! StrategyUISchema와 FragmentRegistry를 조합하여
//! 프론트엔드에서 렌더링할 수 있는 완전한 SDUI JSON을 생성합니다.

use serde_json::{json, Value};
use std::sync::Arc;
use trader_core::{FieldSchema, SchemaFragment, StrategyUISchema};

use crate::FragmentRegistry;

/// 스키마 조합기.
///
/// StrategyUISchema + FragmentRegistry → 완성된 SDUI JSON
pub struct SchemaComposer {
    registry: Arc<FragmentRegistry>,
}

impl SchemaComposer {
    /// 새로운 SchemaComposer를 생성합니다.
    pub fn new(registry: Arc<FragmentRegistry>) -> Self {
        Self { registry }
    }

    /// 기본 FragmentRegistry를 사용하는 SchemaComposer를 생성합니다.
    pub fn with_default_registry() -> Self {
        Self::new(Arc::new(FragmentRegistry::with_builtins()))
    }

    /// StrategyUISchema를 완성된 SDUI JSON으로 변환합니다.
    ///
    /// # Arguments
    /// * `strategy_schema` - 전략 UI 스키마
    ///
    /// # Returns
    /// 프론트엔드에서 렌더링할 수 있는 완전한 SDUI JSON
    pub fn compose(&self, strategy_schema: &StrategyUISchema) -> Value {
        let mut sections = Vec::new();

        // Fragment 섹션 추가
        for frag_ref in &strategy_schema.fragments {
            if let Some(fragment) = self.registry.get(&frag_ref.id) {
                sections.push(self.fragment_to_section(fragment, frag_ref.required));
            }
        }

        // 커스텀 필드 섹션 (있는 경우)
        if !strategy_schema.custom_fields.is_empty() {
            sections.push(self.custom_fields_section(&strategy_schema.custom_fields));
        }

        json!({
            "strategy_id": strategy_schema.id,
            "name": strategy_schema.name,
            "description": strategy_schema.description,
            "category": strategy_schema.category,
            "sections": sections
        })
    }

    /// Fragment를 섹션으로 변환합니다.
    fn fragment_to_section(&self, fragment: &SchemaFragment, required: bool) -> Value {
        json!({
            "id": fragment.id,
            "name": fragment.name,
            "description": fragment.description,
            "category": format!("{:?}", fragment.category),
            "required": required,
            "collapsible": !required,
            "fields": fragment.fields.iter().map(|f| self.field_to_json(f)).collect::<Vec<_>>()
        })
    }

    /// 커스텀 필드를 섹션으로 변환합니다.
    fn custom_fields_section(&self, fields: &[FieldSchema]) -> Value {
        json!({
            "id": "custom",
            "name": "커스텀 설정",
            "description": "전략별 고유 설정",
            "required": false,
            "collapsible": true,
            "fields": fields.iter().map(|f| self.field_to_json(f)).collect::<Vec<_>>()
        })
    }

    /// FieldSchema를 JSON으로 변환합니다.
    fn field_to_json(&self, field: &FieldSchema) -> Value {
        let mut field_json = json!({
            "name": field.name,
            "type": format!("{:?}", field.field_type).to_lowercase(),
            "label": field.label,
            "required": field.required,
        });

        // 선택적 필드 추가
        if let Some(description) = &field.description {
            field_json["description"] = json!(description);
        }

        if let Some(default) = &field.default {
            field_json["default"] = default.clone();
        }

        if let Some(min) = field.min {
            field_json["min"] = json!(min);
        }

        if let Some(max) = field.max {
            field_json["max"] = json!(max);
        }

        if !field.options.is_empty() {
            field_json["options"] = json!(field.options);
        }

        if let Some(condition) = &field.condition {
            field_json["condition"] = json!(condition);
        }

        field_json
    }

    /// Fragment 카탈로그를 JSON으로 반환합니다.
    ///
    /// 사용 가능한 모든 Fragment 목록을 프론트엔드에 제공합니다.
    pub fn get_fragment_catalog(&self) -> Value {
        let fragments: Vec<Value> = self
            .registry
            .list_all()
            .iter()
            .map(|frag| {
                json!({
                    "id": frag.id,
                    "name": frag.name,
                    "description": frag.description,
                    "category": format!("{:?}", frag.category),
                    "dependencies": frag.dependencies,
                    "fields": frag.fields.iter().map(|f| self.field_to_json(f)).collect::<Vec<_>>()
                })
            })
            .collect();

        json!({
            "fragments": fragments
        })
    }

    /// 카테고리별 Fragment 목록을 JSON으로 반환합니다.
    pub fn get_fragments_by_category(&self, category: trader_core::FragmentCategory) -> Value {
        let fragments: Vec<Value> = self
            .registry
            .list_by_category(category)
            .iter()
            .map(|frag| {
                json!({
                    "id": frag.id,
                    "name": frag.name,
                    "description": frag.description,
                    "category": format!("{:?}", frag.category),
                    "fields": frag.fields.iter().map(|f| self.field_to_json(f)).collect::<Vec<_>>()
                })
            })
            .collect();

        json!({
            "category": format!("{:?}", category),
            "fragments": fragments
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trader_core::{FieldType, FragmentRef, StrategyUISchema};

    #[test]
    fn test_compose_basic_schema() {
        let composer = SchemaComposer::with_default_registry();

        let schema = StrategyUISchema::new("test_strategy", "테스트 전략", "single_asset")
            .with_description("테스트용 전략")
            .with_fragment(FragmentRef::required("indicator.rsi"));

        let json = composer.compose(&schema);

        assert_eq!(json["strategy_id"], "test_strategy");
        assert_eq!(json["name"], "테스트 전략");
        assert!(json["sections"].is_array());

        let sections = json["sections"].as_array().unwrap();
        assert_eq!(sections.len(), 1); // RSI fragment만
        assert_eq!(sections[0]["id"], "indicator.rsi");
        assert_eq!(sections[0]["required"], true);
    }

    #[test]
    fn test_compose_with_custom_fields() {
        let composer = SchemaComposer::with_default_registry();

        let schema = StrategyUISchema::new("test_strategy", "테스트 전략", "single_asset")
            .with_fragment(FragmentRef::required("indicator.rsi"))
            .with_custom_field(FieldSchema {
                name: "cooldown_candles".to_string(),
                field_type: FieldType::Integer,
                label: "쿨다운 캔들 수".to_string(),
                default: Some(json!(5)),
                min: Some(0.0),
                max: Some(100.0),
                required: true,
                ..Default::default()
            });

        let json = composer.compose(&schema);

        let sections = json["sections"].as_array().unwrap();
        assert_eq!(sections.len(), 2); // RSI fragment + custom fields

        // 커스텀 필드 섹션 확인
        let custom_section = &sections[1];
        assert_eq!(custom_section["id"], "custom");
        assert_eq!(custom_section["name"], "커스텀 설정");

        let fields = custom_section["fields"].as_array().unwrap();
        assert_eq!(fields[0]["name"], "cooldown_candles");
        assert_eq!(fields[0]["label"], "쿨다운 캔들 수");
    }

    #[test]
    fn test_fragment_catalog() {
        let composer = SchemaComposer::with_default_registry();
        let catalog = composer.get_fragment_catalog();

        assert!(catalog["fragments"].is_array());
        let fragments = catalog["fragments"].as_array().unwrap();
        assert!(fragments.len() > 0);

        // RSI fragment가 있는지 확인
        let rsi_fragment = fragments.iter().find(|f| f["id"] == "indicator.rsi");
        assert!(rsi_fragment.is_some());
    }

    #[test]
    fn test_fragments_by_category() {
        let composer = SchemaComposer::with_default_registry();
        let indicators =
            composer.get_fragments_by_category(trader_core::FragmentCategory::Indicator);

        assert!(indicators["fragments"].is_array());
        let fragments = indicators["fragments"].as_array().unwrap();
        assert!(fragments.len() >= 5); // RSI, MACD, BB, MA, ATR

        // 모든 fragment가 Indicator 카테고리인지 확인
        for frag in fragments {
            assert_eq!(frag["category"], "Indicator");
        }
    }

    #[test]
    fn test_optional_fragment() {
        let composer = SchemaComposer::with_default_registry();

        let schema = StrategyUISchema::new("test_strategy", "테스트 전략", "single_asset")
            .with_fragment(FragmentRef::required("indicator.rsi"))
            .with_fragment(FragmentRef::optional("filter.route_state"));

        let json = composer.compose(&schema);

        let sections = json["sections"].as_array().unwrap();
        assert_eq!(sections.len(), 2);

        // 첫 번째는 필수
        assert_eq!(sections[0]["required"], true);
        assert_eq!(sections[0]["collapsible"], false);

        // 두 번째는 선택적
        assert_eq!(sections[1]["required"], false);
        assert_eq!(sections[1]["collapsible"], true);
    }
}
