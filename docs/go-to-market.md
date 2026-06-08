# Strategy & Open-Source Approach

This document explains *why* AION-MEDSAFE is open source, how the project intends
to create value responsibly, and where it fits in the Medicaid program-integrity
landscape. It is written for prospective adopters, contributors, and partners.

**One line:** AION-MEDSAFE aims to make *verifiable, chain-of-custody evidence* an
open, interoperable standard — and to help integrity teams produce defensible
case files — rather than to compete as another closed scoring product.

---

## 1. Where it fits in the landscape

Medicaid program-integrity and fraud analytics is well served by established
vendors — among them Gainwell, Optum, Conduent, SAS, LexisNexis, and IBM — with
deep state relationships, certified systems, and scale. AION-MEDSAFE is **not**
positioned to replace those systems, and a single open-source project should not
pretend otherwise.

What the category under-emphasizes is **defensibility**: cryptographically
verifiable provenance and evidence that holds up in an administrative hearing or a
courtroom. That is the capability AION-MEDSAFE leads with, and it is **complementary**
to existing scoring/analytics platforms rather than a substitute for them.

| Established capability (incumbents) | What AION-MEDSAFE adds |
|---|---|
| Large-scale claims & analytics, scoring models | Tamper-evident chain of custody over every source, rule, and decision |
| State MMIS / fiscal-agent integration | An open, portable evidence-packet format any tool can produce/verify |
| Mature delivery & support organizations | Hearing-grade, independently re-verifiable case files |

## 2. Why open source

For a chain-of-custody tool, openness is a feature, not a giveaway:

- **Credibility.** "Don't trust us — read the verifier." Auditable code is more
  defensible in legal and oversight settings than a black box.
- **Interoperability.** If the evidence format is open, every tool can emit and
  check it. That benefits investigators regardless of which analytics vendor they
  use.
- **Lower adoption friction.** Agencies can evaluate and run an auditable tool
  without a large procurement, then engage for support/integration as needs grow.
- **Trust through transparency.** Methods, limitations, and provenance are all in
  the open.

## 3. The standard: Sealed Evidence Packet (SEP)

The core contribution is an **open specification** — the
[Sealed Evidence Packet](spec/sealed-evidence-packet.md) — for tamper-evident,
independently verifiable evidence. The aspiration is to be a neutral, widely
implemented format (analogous to how SARIF standardized static-analysis results):
something procurements and oversight guidance can *cite*, and that any vendor can
implement. Standard authorship through openness is more durable and more useful
than secrecy.

## 4. Open-core boundary

| Open (the standard + the trust core) | Commercial services (sustainability) |
|---|---|
| SEP format + the verifier | Data pipelines & source connectors |
| Detection engine + CLI | Analyst workflow + UI |
| Test suites, audit tooling, docs | Hosting / SLA / support |
| | Managed pilots, integration, training/certification |

Revenue, where it exists, comes from services and operations around the tool —
not from keeping the code or the format secret.

## 5. Licensing

| Component | License | Rationale |
|---|---|---|
| Format + spec + verifier + engine | **MIT OR Apache-2.0** (dual) | Maximize adoption of the standard; matches the source SPDX headers and the `aion-context` dependency |
| A future hosted service, if built | **AGPL-3.0** (candidate) | Keep improvements to a network service open and shared |
| Name / logo | Trademark (as applicable) | Protect identity under permissive code |

## 6. A timely, real need

Program-integrity outcomes are under scrutiny. As publicly reported, Hawaii's
Medicaid Fraud Control Unit was decertified in 2026 after years of difficulty
translating effort into indictments and convictions — fundamentally a
**defensibility and evidence** challenge, not a lack of data. That is precisely
the problem an open, auditable, chain-of-custody tool is designed to help with:
turning leads into evidence files that can withstand challenge. The respectful,
useful contribution here is to *help close that gap*, in partnership with the
people doing the work.

## 7. How the project intends to be useful

1. **Publish the open standard and a working reference implementation** (done in
   prototype form).
2. **Offer no-cost evaluation** to integrity units rebuilding their tooling —
   leads-only, public data, no procurement required.
3. **Earn one well-documented, defensible case file** as a concrete proof point.
4. **Invite independent implementations** of SEP and feedback on the spec.
5. **Partner, not just compete** — the provenance/evidence layer can strengthen an
   existing analytics platform or a prime's offering.

## 8. Where opportunities are posted

- Federal: SAM.gov (Contract Opportunities), grants.gov.
- Hawaii Med-QUEST: published solicitations (RFIs typically precede RFPs — the
  moment to shape requirements toward verifiable provenance).
- Hawaii statewide procurement: HIePRO (smaller solicitations), HANDS (awards).

## 9. Honest limitations

- **Open source is not, by itself, adoption or revenue.** Real uptake needs a
  champion and hands-on support; value capture lives in services and operations.
- **A widely adopted format may be implemented by others.** That is a success for
  the standard; the project sustains itself through stewardship, services, and
  expertise rather than exclusivity.
- **Today's outputs are provider-integrity leads, not billing findings** — claims
  analysis is out of scope until claims/MMIS data and the required privacy
  controls are in place.

## 10. Realistic outcomes

In rough order of likelihood for a small project:

1. **Authority & credibility** in verifiable-evidence tooling.
2. **A reference deployment** with an integrity unit.
3. **Standard adoption** — SEP cited/implemented beyond this project.
4. **A sustainable open-core offering** (services/hosting).

Open source most directly accelerates 1–3.

---

*All capability claims reflect the current working system over public data.
Contributions and independent SEP implementations are welcome.*
