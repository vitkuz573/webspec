# FunPay WebSpec Comparison Report

**Date:** 2026-07-25
**URL:** https://funpay.com
**Auto-generated spec:** `/tmp/auto-spec.yaml`
**Hand-written spec:** `/home/vitaly/projects/funpay-spec/spec/funpay.yaml`

---

## Executive Summary

The auto-analyzer detected **15 entities** but **none** match the 19 semantic entities defined in the hand-written spec. The analyzer detected low-level CSS classes and structural elements instead of meaningful domain entities like `Game`, `Offer`, `User`, `Order`, etc.

| Metric | Value |
|--------|-------|
| **Entity Coverage** | 0/19 entities detected (0%) |
| **Field Coverage** | 0/ fields detected (0%) |
| **Selector Accuracy** | N/A |
| **Type Accuracy** | N/A |

---

## Auto-Detected Entities (15)

The analyzer identified these entities based on DOM structure:

| # | Entity Name | Items | Confidence | Description |
|---|-------------|-------|------------|-------------|
| 1 | `button.btn.btn-primary` | 14 | 60% | Primary buttons (likely login/register) |
| 2 | `ul.hidden.list-inline` | 22 | 60% | Hidden list elements (Aion servers) |
| 3 | `ul.list-inline` | 432 | 60% | Visible list elements (game categories) |
| 4 | `div.game-title.hidden` | 3 | 71% | Hidden game titles (Aion variants) |
| 5 | `div.game-title` | 831 | 60% | Game title elements |
| 6 | `button.btn.btn-gray` | 22 | 60% | Gray buttons (server filters) |
| 7 | `Col-xs-6Entity` | 801 | 68% | 6-column grid items (game cards) |
| 8 | `Promo-game-list-titleEntity` | 38 | 68% | Letter headers (A-Z) |
| 9 | `Row-10Entity` | 38 | 68% | Row containers for games |
| 10 | `Col-xs-12Entity` | 5 | 68% | Full-width columns (featured games) |
| 11 | `Btn-roundEntity` | 3 | 59% | Round buttons |
| 12 | `Btn-grayEntity` | 3 | 68% | Gray button group |
| 13 | `Icon-barEntity` | 3 | 59% | Icon bars |
| 14 | `HiddenEntity` | 3 | 71% | Hidden elements |
| 15 | `List-inlineEntity` | 3 | 71% | List-inline groups |

---

## Hand-Written Entities (19)

The hand-written spec defines these semantic entities:

| # | Entity | Fields | Key Selectors |
|---|--------|--------|---------------|
| 1 | `Game` | 4 | `.game-title`, `.game-icon` |
| 2 | `Category` | 5 | `.category-item`, `.cat-title`, `.cat-count` |
| 3 | `SubCategory` | 4 | `.subcategory-item`, `.cat-title` |
| 4 | `GameServer` | 5 | `.server-item`, `.server-name`, `.server-platform` |
| 5 | `Offer` | 9 | `.tc-item`, `.tc-server`, `.tc-price`, `.tc-desc-text` |
| 6 | `OfferLot` | 4 | `.tc-server`, `.tc-price`, `.tc-desc-text` |
| 7 | `User` | 6 | `.profile-user-id`, `.profile-title`, `.profile-avatar` |
| 8 | `LotItem` | 3 | `.tc-item-text`, `.tc-item img` |
| 9 | `Seller` | 7 | `.seller-info`, `.seller-name`, `.seller-rating`, `.seller-reviews` |
| 10 | `Lot` | 6 | `.tc-game`, `.tc-server`, `.tc-price` |
| 11 | `Order` | 8 | `.order-item`, `.order-game`, `.order-status`, `.order-price` |
| 12 | `Transaction` | 8 | `.transaction-item`, `.transaction-amount`, `.transaction-date` |
| 13 | `Chat` | 6 | `.chat-item`, `.chat-user`, `.chat-last-message`, `.chat-unread` |
| 14 | `ChatMessage` | 5 | `.msg`, `.msg-text`, `.msg-date` |
| 15 | `Review` | 6 | `.review-item`, `.review-author`, `.review-text`, `.review-rating` |
| 16 | `Notification` | 7 | `.notification-item`, `.notification-title`, `.notification-text` |
| 17 | `Search` | 4 | `.search-input`, `.search-results-count`, `.tc-item` |
| 18 | `Settings` | 6 | `.settings-email`, `.settings-phone`, `.settings-language` |
| 19 | `GameCategory` | 3 | `.category-item`, `.cat-title` |

---

## Entity-by-Entity Comparison

### 1. Game

| Field | Hand-Written Selector | Auto-Detected | Match |
|-------|----------------------|---------------|-------|
| `id` | `.game-title` → `data-game-id` | Not detected | ❌ |
| `title` | `.game-title` (text) | `div.game-title` detected (831 items) | ⚠️ PARTIAL |
| `icon_url` | `.game-icon` → `src` | Not detected | ❌ |
| `url` | `.game-title` → `href` | Not detected | ❌ |

**Verdict:** Analyzer found `.game-title` elements but only extracted `id` from `data-game-id` attribute. Missing: `icon_url`, `url`, proper semantic naming.

---

### 2. Category

| Field | Hand-Written Selector | Auto-Detected | Match |
|-------|----------------------|---------------|-------|
| `id` | `.category-item` → `data-category-id` | Not detected | ❌ |
| `title` | `.cat-title` | Not detected | ❌ |
| `url` | `.category-item a` → `href` | Not detected | ❌ |
| `game_id` | (nested context) | Not detected | ❌ |
| `offers_count` | `.cat-count` | Not detected | ❌ |

**Verdict:** Not detected. The analyzer focused on `.list-inline` elements instead of `.category-item`.

---

### 3. SubCategory

| Field | Hand-Written Selector | Auto-Detected | Match |
|-------|----------------------|---------------|-------|
| `id` | `.subcategory-item` → `data-category-id` | Not detected | ❌ |
| `name` | `.subcategory-item .cat-title` | Not detected | ❌ |
| `url` | `.subcategory-item a` → `href` | Not detected | ❌ |
| `parent_id` | `.subcategory-item` → `data-parent-id` | Not detected | ❌ |

**Verdict:** Not detected. Subcategories not visible on main page.

---

### 4. GameServer

| Field | Hand-Written Selector | Auto-Detected | Match |
|-------|----------------------|---------------|-------|
| `id` | `.server-item` → `data-server-id` | Not detected | ❌ |
| `name` | `.server-item .server-name` | Not detected | ❌ |
| `game_id` | `.server-item` → `data-game-id` | Not detected | ❌ |
| `platform` | `.server-item .server-platform` | Not detected | ❌ |
| `offers_count` | `.server-item .server-count` | Not detected | ❌ |

**Verdict:** Not detected. Server items not visible on main page.

---

### 5. Offer

| Field | Hand-Written Selector | Auto-Detected | Match |
|-------|----------------------|---------------|-------|
| `id` | `.tc-item` → `data-order` | Not detected | ❌ |
| `seller_id` | `.tc-item` → `data-user-id` | Not detected | ❌ |
| `server` | `.tc-server` | Not detected | ❌ |
| `price` | `.tc-price` | Not detected | ❌ |
| `currency` | `.tc-price .currency` | Not detected | ❌ |
| `description` | `.tc-desc-text` | Not detected | ❌ |
| `sale_type` | `.tc-item` → `data-mark` | Not detected | ❌ |
| `item_count` | `.tc-item` → `data-lot-size` | Not detected | ❌ |
| `image_url` | `.tc-item img` → `src` | Not detected | ❌ |

**Verdict:** Not detected. Offer elements not present on main page (only on category pages).

---

### 6-19. Remaining Entities

**OfferLot, User, LotItem, Seller, Lot, Order, Transaction, Chat, ChatMessage, Review, Notification, Search, Settings, GameCategory** — **ALL NOT DETECTED**

**Reason:** These entities require navigation to specific pages:
- User/Seller → `/users/{id}/`
- Order → `/orders/`
- Chat → `/chats/`
- Transaction → `/wallet/transactions/`
- Notification → `/notifications/`
- Settings → `/settings/`
- Review → `/users/{id}/reviews/`
- Search → `/search/?q=...`

The analyzer only fetched the main page (`/lots/`).

---

## Gap Analysis

### Why Did the Analyzer Miss Everything?

1. **Single-page limitation:** Only fetched main page, not sub-pages
2. **Structural detection:** Focused on DOM patterns, not semantic meaning
3. **No data-attribute awareness:** Didn't extract `data-*` attributes meaningfully
4. **No page-type classification:** Didn't identify page types (list, detail, form)
5. **No entity relationship mapping:** Didn't connect nested entities

### What the Analyzer Got Right

1. ✅ Found `.game-title` elements (831 games)
2. ✅ Found `.list-inline` elements (game categories)
3. ✅ Correctly identified item counts
4. ✅ Detected confidence levels

### Critical Missing Features

| Feature | Status | Impact |
|---------|--------|--------|
| Multi-page crawling | ❌ Missing | Can't detect most entities |
| Semantic entity naming | ❌ Missing | Uses CSS classes as names |
| Data-attribute extraction | ❌ Partial | Only extracts `data-id` |
| Page-type classification | ❌ Missing | Can't identify page context |
| Entity relationships | ❌ Missing | No parent-child mapping |
| Transform detection | ❌ Missing | No `parse_price`, `parse_date` |
| Enum detection | ❌ Missing | No `OrderStatus`, `ChatType` |
| Auth detection | ❌ Missing | No cookie/auth info |
| Rate limit detection | ❌ Missing | No rate limiting info |

---

## Recommendations

### Immediate Fixes

1. **Add multi-page crawling** — Navigate to category pages, user profiles, etc.
2. **Implement semantic naming** — Map CSS patterns to domain entities
3. **Extract all data-attributes** — Not just `data-id`
4. **Add page-type classifier** — Identify list, detail, form pages

### Advanced Features

1. **Entity relationship graph** — Map parent-child entity relationships
2. **Transform inference** — Detect price parsing, date parsing patterns
3. **Enum value extraction** — Extract possible values from UI elements
4. **Auth flow detection** — Identify required cookies/tokens
5. **Rate limit detection** — Monitor request patterns

### Priority Order

1. Multi-page crawling (blocks 90% of entity detection)
2. Semantic entity naming (makes output usable)
3. Data-attribute extraction (captures IDs and metadata)
4. Page-type classification (context for entities)

---

## Conclusion

The current analyzer is **not suitable** for generating production-ready webspecs from real websites. It produces structural DOM analysis rather than semantic entity specifications.

**To match the hand-written spec quality, the analyzer needs:**
- Multi-page crawling capability
- Semantic entity recognition
- Data-attribute extraction
- Page-type classification
- Entity relationship mapping
- Transform/enum detection
- Auth/rate-limit detection

**Estimated effort to reach parity:** 2-3 weeks of development.

---

*Report generated by comparing `/tmp/auto-spec.yaml` with `/home/vitaly/projects/funpay-spec/spec/funpay.yaml`*
