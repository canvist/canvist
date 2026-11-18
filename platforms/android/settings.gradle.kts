// GitHub-hosted runners share addresses that Maven Central answers with HTTP
// 429. Gradle treats that as a failed resolution, so the Android publish job
// dies while configuring the Maven Publish plugin. This is Maven Central's
// public Cloud Storage mirror; it serves the same artifacts. `pluginManagement`
// is compiled on its own, so the repository is declared in both places.

pluginManagement {
    repositories {
        google()
        maven {
            name = "MavenCentral"
            url = uri("https://maven-central.storage-download.googleapis.com/maven2/")
        }
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode = RepositoriesMode.FAIL_ON_PROJECT_REPOS
    repositories {
        google()
        maven {
            name = "MavenCentral"
            url = uri("https://maven-central.storage-download.googleapis.com/maven2/")
        }
    }
}

rootProject.name = "canvist-android"
include(":canvist", ":sample")
