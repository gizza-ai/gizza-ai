## About this tool

Generate a fixed-rate loan amortization schedule from a principal balance, annual rate, term, and payment frequency. The calculator shows the scheduled payment, each period's interest and principal, the remaining balance, total interest, total paid, and how much time or interest an extra payment saves.

Worked example: enter `300000` for `loan_amount`, `6` for `annual_interest_rate_percent`, `30` for `loan_years`, `monthly` for `payment_frequency`, and `2026-01-01` for `start_date`. The first payment is `$1,798.65`; the first row shows `$1,500.00` of interest, `$298.65` of principal, and a `$299,701.35` balance.

Use `extra_payment` for a recurring additional principal payment. Use `extra_one_time` with `extra_one_time_period` for a lump sum in a specific period. Choose `annual` in `schedule_view` to collapse the detailed table into one row per loan year, or choose `csv` / `json` in `format` when you want spreadsheet or programmatic output.

Limits and edge cases: this is deterministic fixed-rate math, not financial advice. It does not model adjustable rates, escrow, taxes, insurance, late fees, prepayment penalties, compounding conventions that differ from the selected payment frequency, or lender-specific rounding. The schedule is capped at 5,000 payments; weekly 100-year loans exceed that cap, while biweekly 100-year loans are accepted. Blank dates use `2026-01-01` so examples are reproducible.

## FAQ

<details>
<summary>What does the payment amount mean?</summary>

`payment_per_period` is the level scheduled principal-and-interest payment for the selected frequency. Extra payments are added on top of that scheduled payment and go directly to principal.

</details>

<details>
<summary>How do extra payments affect the schedule?</summary>

A recurring `extra_payment` is applied every period, and `extra_one_time` is applied once at `extra_one_time_period`. The result reports `interest_saved` and `payments_saved` compared with the same loan without those extras.

</details>

<details>
<summary>Why choose annual view?</summary>

The period view is useful for auditing exact payments. The annual view groups each loan year into totals for paid, principal, interest, extra principal, and ending balance, which is easier to scan or paste into a summary model.

</details>

<details>
<summary>Can I export the table?</summary>

Yes. Set `format` to `csv` for spreadsheet rows or `json` for the full structured result. The page also offers copy and download controls for the generated output.

</details>
