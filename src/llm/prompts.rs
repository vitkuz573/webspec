pub fn build_field_naming_prompt(page_title: &str, html_snippets: &str) -> String {
    format!(
        r#"You are analyzing HTML elements from the web page "{page_title}".

Below are CSS selectors found on the page, each with sample text values extracted from the actual page.

{html_snippets}

YOUR TASK: For EACH selector above, assign a meaningful field name.

RULES:
- Field names MUST be descriptive English words in snake_case
- GOOD examples: "price", "seller_name", "item_title", "server_status", "game_name", "avatar_url"
- BAD examples: "btn-group-xs", "col-md-6", "data-id", "class" — these are CSS/HTML artifacts, NOT field names
- Look at the SAMPLE VALUES to understand what the field actually contains
- Type must be one of: String, f64, u32, bool, Url, Timestamp
- If the sample contains a price with currency symbol (like "1 234 ₽" or "$99"), type is "Price"
- If it looks like a numeric ID, type is "u32"
- If it looks like a URL/link, type is "Url"

Return ONLY a JSON array (no wrapper object, no explanation):
[
  {{"selector": ".product-title", "name": "product_title", "type": "String"}},
  {{"selector": ".price", "name": "price", "type": "Price"}},
  {{"selector": "[data-id]", "name": "item_id", "type": "u32"}}
]

IMPORTANT: Return ONLY the JSON array. No text before or after."#
    )
}

pub fn build_entity_grouping_prompt(page_title: &str, fields_json: &str) -> String {
    format!(
        r#"You are grouping fields from the web page "{page_title}" into logical data models.

Below are the fields extracted from the page (each has a "name" attribute):

{fields_json}

YOUR TASK: Group these fields into meaningful entities.

CRITICAL RULE: You MUST use the EXACT "name" values from the input above. Do NOT invent new field names. If the input says a field is named "price_value", your entity must use "price_value", not "price".

RULES:
- Entity names MUST be PascalCase English nouns (Offer, Product, User, Game, etc.)
- The "fields" array in each entity MUST contain the exact "name" values from the input
- list_selector is the CSS selector that repeats for each instance (null if single-instance)

Return ONLY a JSON array:
[
  {{"name": "Offer", "description": "A listing on the site", "list_selector": ".tc-item", "fields": ["price_value", "seller_name"]}}
]

IMPORTANT: Return ONLY the JSON array. No text before or after."#
    )
}

pub fn build_enum_detection_prompt(sample_values: &str) -> String {
    format!(
        r#"You are detecting enum fields from sample values of web page fields.

Here are fields with their sample values:

{sample_values}

For each field, determine if it represents an enumerated type (a fixed set of possible values).

Return ONLY a JSON array:
[
  {{"field_name": "name", "type_name": "EnumTypeName", "values": ["value1", "value2"], "description": "what this enum represents"}}
]

Rules:
- A field is an enum if it has <= 10 unique values across all items
- Enum values should be snake_case
- Type names should be PascalCase
- Return ONLY the JSON array. No text before or after."#
    )
}
