pub fn build_field_naming_prompt(page_title: &str, html_snippets: &str) -> String {
    format!(
        r#"You are analyzing HTML elements from the web page "{page_title}".

Below are CSS selectors found on the page, each with sample text values extracted from the actual page.

{html_snippets}

YOUR TASK: For EACH selector above, assign a meaningful field name.

CRITICAL SELECTOR RULES — READ CAREFULLY:
- The "selector" field in your response MUST be one of the EXACT selectors listed above (copy it character-for-character)
- NEVER invent new CSS selectors — only use the ones provided in the input
- NEVER put URLs (like "https://..."), pure numbers (like "1.099796"), or text content into the selector field
- The selector must be a valid CSS selector (tag, class, id, or attribute selector)
- If you cannot determine a good selector for a field, skip it entirely — do not make one up

GOOD selector examples from input: ".product-title", ".price", "[data-id]", "a.profile-link"
BAD selector examples (NEVER produce these): "div.1.099796", "div.https://example.com/", ".12345", "some random text"

Field naming rules:
- Field names MUST be descriptive English words in snake_case
- GOOD field name examples: "price", "seller_name", "item_title", "server_status", "game_name", "avatar_url"
- BAD field name examples: "btn-group-xs", "col-md-6", "data-id", "class" — these are CSS/HTML artifacts, NOT field names
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

pub fn build_transform_prompt(entities_json: &str) -> String {
    format!(
        r#"Given these web entities and their fields, determine which fields need transform functions.

{entities_json}

Available transforms:
- parse_price: extracts numeric value from text with currency symbols (e.g., "1 234 ₽" → 1234.0)
- parse_date: parses date strings into ISO format (e.g., "25.01.2026" → "2026-01-25")
- parse_id_from_url: extracts numeric ID from URL path (e.g., "/users/123/" → 123)

Return ONLY a JSON array:
[
  {{"field": "field_name", "transform": "parse_price"}},
  {{"field": "date_field", "transform": "parse_date"}}
]

Only include fields that NEED a transform. Skip fields that don't need one.

IMPORTANT: Return ONLY the JSON array. No text before or after."#
    )
}

pub fn build_enum_prompt(entities_json: &str) -> String {
    format!(
        r#"Given these web entities, detect which fields represent enumerated types (fixed set of possible values).

{entities_json}

For each enum field, provide the possible values.

Return ONLY a JSON array:
[
  {{
    "name": "StatusEnum",
    "field": "status_field",
    "values": ["active", "completed", "cancelled"],
    "description": "Possible statuses"
  }}
]

Rules:
- Only detect fields with a small fixed set of possible values (<= 10 unique values)
- Enum values should be snake_case
- Type names should be PascalCase
- Return ONLY the JSON array. No text before or after."#
    )
}

pub fn build_url_priority_prompt(urls_with_context: &str) -> String {
    format!(
        r#"You are analyzing URLs from a website to find pages with the most structured data.

Here are the URLs found on the site, with context about where each link appears:

{urls_with_context}

Your task: Prioritize these URLs for crawling. Pages with structured/repeated data (product listings, user profiles, order lists) are most valuable. Pages with static content (login, privacy policy, FAQ) are least valuable.

Return ONLY a JSON array, sorted from most to least valuable:
[
  {{"url": "...", "score": 95, "reason": "likely product listing page"}},
  {{"url": "...", "score": 80, "reason": "category page with item grid"}},
  ...
]

Score 0-100. Be concise in reasons."#
    )
}

pub fn build_full_spec_prompt(
    url: &str,
    page_titles: &str,
    html_snippets: &str,
    data_attributes: &str,
    url_patterns: &str,
) -> String {
    format!(
        r#"You are generating a webspec YAML specification for the website at {url}.

## Page titles found:
{page_titles}

## HTML structure (repeated elements, data attributes, text content):
{html_snippets}

## data-* attributes found:
{data_attributes}

## URL patterns detected:
{url_patterns}

Your task: Generate a complete webspec YAML specification.

Rules:
1. Entity names: PascalCase English nouns describing what the data represents (e.g., Product, User, Review, Order)
2. Field names: snake_case English describing the data (e.g., price, title, seller_name)
3. Types: String, f64, u32, bool, Url, Price (f64 with currency), Timestamp
4. Selectors: Use EXACTLY the selectors from the input HTML snippets. Do not invent new selectors.
5. Transforms: parse_price, parse_date, parse_id_from_url, or null
6. Each entity needs: name, description, list_selector (or null for single-instance), fields
7. Each field needs: name, type, selector, attribute (or null), transform (or null), nullable (true/false)
8. Groups of fields that appear together in the same repeated DOM element = one entity

Return ONLY valid YAML in this exact format:
```yaml
version: "1.0"
name: <snake_case site name derived from page titles>
base_url: {url}
types:
  String:
    rust: String
  f64:
    rust: f64
  u32:
    rust: u32
  bool:
    rust: bool
  Url:
    rust: String
    newtype: true
  Timestamp:
    rust: String
  Price:
    rust: f64
    newtype: true
enums:
  <EnumName>:
    values:
      <value>: <description>
entities:
  <EntityName>:
    description: <what this entity represents>
    list_selector: <CSS selector for repeated instances, or null>
    fields:
      <field_name>:
        type: <String|f64|u32|bool|Url|Price|Timestamp|EnumName>
        selector: <EXACT CSS selector from input>
        attribute: <HTML attribute name, or null>
        transform: <parse_price|parse_date|parse_id_from_url|null>
        nullable: <true|false>
```

Be thorough. Detect ALL entities and fields visible in the HTML structure."#
    )
}
