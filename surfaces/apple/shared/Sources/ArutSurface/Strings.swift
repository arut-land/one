import Foundation

/// The core names an error by its Fluent message id and hands over the
/// arguments that id takes; no sentence crosses the boundary and the core still
/// learns no locale (ADR 0016, ADR 0022). Resolution happens here, in the same
/// String Catalog and with the same negotiation as every other label.
enum Strings {
    static func localized(_ key: String, arguments: [String]) -> String {
        let format = String(
            localized: String.LocalizationValue(key),
            table: "Localizable",
            bundle: .module
        )
        guard !arguments.isEmpty else { return format }
        return String(format: format, arguments: arguments.map { $0 as CVarArg })
    }
}
