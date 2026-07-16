import UIKit

@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?
    private let destinationLabel = UILabel()

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        let window = UIWindow(frame: UIScreen.main.bounds)
        let rootViewController = UIViewController()
        rootViewController.view.backgroundColor = UIColor(
            red: 0.035,
            green: 0.075,
            blue: 0.14,
            alpha: 1
        )
        window.rootViewController = rootViewController

        let title = UILabel()
        title.text = "DeepLink Lab Fixture"
        title.textColor = .white
        title.font = .systemFont(ofSize: 28, weight: .bold)
        title.textAlignment = .center

        destinationLabel.textColor = UIColor(red: 0.42, green: 0.70, blue: 1, alpha: 1)
        destinationLabel.font = .monospacedSystemFont(ofSize: 20, weight: .semibold)
        destinationLabel.textAlignment = .center
        destinationLabel.numberOfLines = 0
        destinationLabel.accessibilityIdentifier = "deeplinklab.destination"

        let note = UILabel()
        note.text = "This app intentionally contains deterministic routing faults."
        note.textColor = UIColor(white: 0.72, alpha: 1)
        note.font = .systemFont(ofSize: 14)
        note.textAlignment = .center
        note.numberOfLines = 0

        let stack = UIStackView(arrangedSubviews: [title, destinationLabel, note])
        stack.axis = .vertical
        stack.spacing = 22
        stack.translatesAutoresizingMaskIntoConstraints = false
        rootViewController.view.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: rootViewController.view.leadingAnchor, constant: 28),
            stack.trailingAnchor.constraint(
                equalTo: rootViewController.view.trailingAnchor,
                constant: -28
            ),
            stack.centerYAnchor.constraint(equalTo: rootViewController.view.centerYAnchor),
        ])

        self.window = window
        window.makeKeyAndVisible()
        record("home")

        if let url = launchOptions?[.url] as? URL {
            route(url, delivery: "cold")
        }
        return true
    }

    func application(
        _ app: UIApplication,
        open url: URL,
        options: [UIApplication.OpenURLOptionsKey: Any] = [:]
    ) -> Bool {
        route(url, delivery: "continuation")
        return true
    }

    private func route(_ url: URL, delivery: String) {
        let family = url.host ?? ""
        let components = URLComponents(url: url, resolvingAgainstBaseURL: false)
        let path = url.path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        let query = components?.queryItems?.first(where: { $0.name == "q" })?.value
        let fragment = components?.fragment

        let destination: String
        switch family {
        case "wrong":
            destination = "home"
        case "query-loss":
            destination = "search"
        case "fragment-loss":
            destination = path
        case "route-collision":
            destination = "user/admin"
        case "warm-stale", "background-ignore":
            return
        case "encoding-collapse":
            destination = path.removingPercentEncoding ?? path
        default:
            destination = canonical(path: path, query: query, fragment: fragment)
        }
        record(destination)
    }

    private func canonical(path: String, query: String?, fragment: String?) -> String {
        var value = path
        if let query { value += "?q=\(query)" }
        if let fragment { value += "#\(fragment)" }
        return value
    }

    private func record(_ destination: String) {
        destinationLabel.text = "Destination: \(destination)"
        destinationLabel.accessibilityValue = destination
        let file = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("destination.txt")
        try? destination.write(to: file, atomically: true, encoding: .utf8)
        NSLog("DEEPLINKLAB_DESTINATION=%@", destination)
    }
}
