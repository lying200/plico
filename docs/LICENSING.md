# Licensing

Plico's original source code is licensed under Apache-2.0; see ../LICENSE.
Third-party code and assets retain their existing licenses and notices.
The owner-supplied Plico logo is excluded from the source-code license grant;
no independent third-party license or trademark permission is inferred.

## Upstream checks before initial GitHub publication

- Rebased reference: `4aec35256d765c217aa0d662b82ac4e860c396fd`.
  [LICENSE.txt](https://github.com/DetachHead/rebased/blob/4aec35256d765c217aa0d662b82ac4e860c396fd/LICENSE.txt)
  contains JetBrains Open-Source Build Terms, not the Apache license text.
  Those terms describe IntelliJ IDEA/PyCharm build products containing
  Apache-2.0 software; Plico does not adopt those product terms as its license.
- The source used for adaptations has Apache-2.0 headers, for example
  [UnifiedDiffViewer.java](https://github.com/DetachHead/rebased/blob/4aec35256d765c217aa0d662b82ac4e860c396fd/platform/diff-impl/src/com/intellij/diff/tools/fragmented/UnifiedDiffViewer.java).
  Rebased's [NOTICE.txt](https://github.com/DetachHead/rebased/blob/4aec35256d765c217aa0d662b82ac4e860c396fd/NOTICE.txt)
  attribution is retained in the root NOTICE.
- JetBrains IntelliJ Community's [NOTICE.txt](https://github.com/JetBrains/intellij-community/blob/master/NOTICE.txt)
  identifies JetBrains copyright. File-specific and third-party terms must
  still be retained; this is not a claim that every JetBrains product or every
  file in its repository has the same license.
- The three vendored GPUI crates declare Apache-2.0 in Cargo.toml and include
  LICENSE-APACHE. Original licenses remain in vendor/.
- Inter, JetBrains Mono and Noto Sans CJK material retain SIL OFL 1.1 notices
  under assets/fonts/. Icon/theme origins and modifications are listed in
  their SOURCE.txt files. Rust dependency licenses are collected by the
  Windows packaging script; they are not relicensed by the project LICENSE.

[Apache-2.0 section 4](https://www.apache.org/licenses/LICENSE-2.0) requires
license distribution, retention of applicable attribution notices and
identification of modified material. Section 6 does not grant trademark
rights. Root NOTICE, original vendored licenses and asset/source records
are retained for these purposes. This review concerns the referenced material,
not a blanket license clearance for future imports.

## Chinese language pack reference (2026-10-28)

The official [JetBrains Chinese language pack](https://plugins.jetbrains.com/plugin/13710-chinese-simplified-language-pack----)
(`com.intellij.zh`, standalone version `232.322`) links to the
[JetBrains User Agreement](https://www.jetbrains.com/legal/docs/toolbox/user/),
not an open-source license. Its Marketplace metadata provides no source-code
URL. We have not established an open-source reuse grant for these translations;
availability for download or bundling with an IDE does not establish one.
The archive is not included in Plico. Short UI terminology was consulted during
the source audit, while Plico wording is reviewed against English resources and
actual operation semantics; this is not a bulk translation import.

This audit uses Rebased commit `245042348669114adb8e24e5b0fc0ce73763f3c4`.
Its root LICENSE.txt still contains JetBrains Open-Source Build Terms;
the inspected GitRebaseEntry.kt declares Apache-2.0 in its file header.
This does not extend that source license to the separately distributed language pack.

The former root LICENSE began with a Zed copyright header copied from an
upstream license file. It now contains the standard Apache terms; the Zed
attribution remains in NOTICE and third-party license records.
