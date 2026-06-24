# Timezone Converter Competitor Analysis & Improvement Plan

This document analyzes the current state of our timezone-convert tool, summarizes competitor features based on research, and outlines our improvement plan.

## Competitor Profiles

### 1. World Time Buddy
* **URL:** `https://www.worldtimebuddy.com`
* **Features:** Aligned hour grids, slider, click-and-drag scheduling slots, warnings for DST changes, reorderable list of regions, widget embeds, shareable URLs.
* **UX Patterns:** Grid-aligned horizontal timelines, drag-to-select range highlighting, dynamic reordering of rows, synchronized column focus.

### 2. Timeanddate World Clock Converter
* **URL:** `https://www.timeanddate.com/worldclock/converter.html`
* **Features:** Comparison of up to 12 locations, hourly slider, color-graded hour blocks (business/leisure/rest), shareable URLs, geo-lookup, event exports (ICS), print-friendly layouts.
* **UX Patterns:** Stacked synchronized tables, hourly slider, color-graded slots, calendar export integrations.

### 3. Savvy Time
* **URL:** `https://savvytime.com`
* **Features:** Synchronized multiple zones, interactive scrubbing slider, work/waking/rest hour colors, calendar integration, customizable aliases, 12h/24h toggle.
* **UX Patterns:** Synchronized time sliders, color-coded timelines, type-ahead autocomplete location search, drag-and-drop list sorting.

### 4. Dateful Time Converter
* **URL:** `https://dateful.com/time-zone-converter`
* **Features:** Live conversions as you type, event link generator translating to visitor's local clock, time calculation (addition/subtraction of durations), natural language parsing (e.g. "tomorrow at 6pm"), 12/24h toggle, bidirectional swap.
* **UX Patterns:** Live preview, autocomplete-search, natural text parsing, bidirectional swap.

---

## Gap Analysis (Gizza vs. Competitors)

### In-Model Gaps (Exposed via WASM/Core Logic)
1. **Single-Target Limit:** Currently, we only convert from *one* zone to *one* other zone. Competitors all support multi-zone comparison (adding multiple locations).
2. **No Meeting Planner Grid:** Competitors provide hour-by-hour grids indicating overlapping business/rest/leisure hours.
3. **Rigid Parsing:** Currently, we only parse strict ISO forms (`YYYY-MM-DD HH:MM:SS`, `YYYY-MM-DD HH:MM`, and `YYYY-MM-DD`). We lack AM/PM parsing and slash separators (e.g. `YYYY/MM/DD`).
4. **No Calendar Export Helper:** We don't generate event details or helper structures for external calendars.

### Out-of-Model Gaps (Web Client/UI Only)
1. **Interactive Slider/Scrubber:** Interactive timeline scrubbing to check offsets.
2. **Dynamic Location Search / Autocomplete:** Auto-suggest list of cities and matching IANA names.

---

## Proposed Improvements

To close the gaps, we will implement the following improvements in `timezone-convert`:

### 1. Core Logic & API Improvements (`core/src/lib.rs` & `src/lib.rs`)
* **Multi-Target Conversion:** Update the `to` field to accept a comma-separated list of target timezones (e.g., `Asia/Tokyo, Europe/London, UTC`).
* **Enhanced JSON Response:**
  * Keep the existing top-level fields (based on the first target in the list) for backwards-compatibility.
  * Add a `targets` list of all converted targets. Each entry will contain:
    * `to_zone`, `to_offset`, `to_pretty`, `to_weekday`, `to_is_dst`, `offset_diff_hours`, `offset_diff_minutes`, `unix`, `to_iso8601`.
  * Add a `meeting_planner` table containing a list of 24 hourly slots starting at the parsed date. Each slot will contain:
    * `hour_index` (0 to 23).
    * `from_time` (formatted local time in source zone).
    * `from_hour` (integer 0-23 in source zone).
    * `from_status` (`"Business"`, `"Leisure"`, or `"Rest"`).
    * `targets`: A list of maps, each containing:
      * `to_zone`.
      * `to_time` (formatted local time in target zone).
      * `to_hour` (integer 0-23 in target zone).
      * `to_status` (`"Business"`, `"Leisure"`, or `"Rest"`).
* **Lenient Date Parsing:** Extend `parse_naive` to support:
  * Slash separators: `YYYY/MM/DD`
  * AM/PM forms: `HH:MM AM/PM` and `HH:MM:SS AM/PM` (case-insensitive, optional space before AM/PM).

### 2. Standalone Web Page UI Improvements (`blocks/timezone-convert/page/`)
* **Dynamic Multi-Zone Inputs & Live Previews:**
  * Add multiple target zones dynamically.
  * Render the meeting planner grid as a beautiful interactive table showing hour overlaps.
  * Implement professional color themes: Emerald/Teal for Business, Amber/Orange for Leisure, Indigo/Slate for Rest.
  * Add a toggle for 12-hour vs 24-hour time representation.
  * Add a copyable meeting summary button.
