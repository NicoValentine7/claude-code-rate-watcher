import Foundation

if CommandLine.arguments.contains("--version") || CommandLine.arguments.contains("-V") {
    print("ccrw \(BuildInfo.currentVersion)")
    exit(0)
}

CCRWApp.main()
