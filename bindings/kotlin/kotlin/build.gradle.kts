plugins {
    kotlin("jvm") version "1.9.21"
    kotlin("plugin.serialization") version "1.9.21"
    `maven-publish`
    signing
}

group = "com.stateset"
version = "1.23.5"

repositories {
    mavenCentral()
}

dependencies {
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.6.2")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.7.3")

    testImplementation(kotlin("test"))
    testImplementation(kotlin("test-junit5"))
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.7.3")
}

kotlin {
    jvmToolchain(11)
}

tasks.test {
    useJUnitPlatform()
    systemProperty("java.library.path", "${project.rootDir}/../../../target/release")
}

// Native library loading support
tasks.jar {
    manifest {
        attributes(
            "Implementation-Title" to "StateSet Embedded Commerce",
            "Implementation-Version" to version
        )
    }

    // Include native libraries
    from("${project.rootDir}/../../../target/release") {
        include("*.so", "*.dylib", "*.dll")
        into("native/")
    }
}

publishing {
    publications {
        create<MavenPublication>("maven") {
            artifactId = "embedded-kotlin"
            from(components["kotlin"])

            pom {
                name.set("StateSet Embedded Commerce")
                description.set("Kotlin bindings for StateSet Embedded Commerce - The SQLite of Commerce")
                url.set("https://github.com/stateset/stateset-icommerce")

                licenses {
                    license {
                        name.set("MIT License")
                        url.set("https://opensource.org/licenses/MIT")
                    }
                    license {
                        name.set("Apache License, Version 2.0")
                        url.set("https://www.apache.org/licenses/LICENSE-2.0")
                    }
                }

                developers {
                    developer {
                        id.set("stateset")
                        name.set("StateSet")
                        email.set("support@stateset.com")
                    }
                }

                scm {
                    connection.set("scm:git:git://github.com/stateset/stateset-icommerce.git")
                    developerConnection.set("scm:git:ssh://github.com/stateset/stateset-icommerce.git")
                    url.set("https://github.com/stateset/stateset-icommerce")
                }
            }
        }
    }

    repositories {
        maven {
            name = "OSSRH"
            url = uri("https://s01.oss.sonatype.org/service/local/staging/deploy/maven2/")
            credentials {
                username = System.getenv("MAVEN_USERNAME")
                password = System.getenv("MAVEN_PASSWORD")
            }
        }
    }
}

signing {
    sign(publishing.publications["maven"])
}
