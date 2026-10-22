package dev.jfcofer.diapason

import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.Parameterized
import pl.leancode.patrol.PatrolJUnitRunner

// The JUnit entry point patrol needs (docs/adr/0026): one parameterised case per Dart test, which
// patrol lists from the app once it is running. A Kotlin port of patrol's Java template, so ktfmt
// formats it like the rest of our Kotlin (docs/adr/0023).
@RunWith(Parameterized::class)
class MainActivityTest(private val dartTestName: String) {
    @Test
    fun runDartTest() {
        runner().runDartTest(dartTestName)
    }

    companion object {
        private fun runner() = InstrumentationRegistry.getInstrumentation() as PatrolJUnitRunner

        @JvmStatic
        @Parameterized.Parameters(name = "{0}")
        fun testCases(): Array<Any> {
            val runner = runner()
            runner.setUp(MainActivity::class.java)
            runner.waitForPatrolAppService()
            return runner.listDartTests()
        }
    }
}
