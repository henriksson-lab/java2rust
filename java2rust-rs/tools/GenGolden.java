import de.aschoerk.java2rust.JavaConverter;

import java.io.PrintStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * Generates golden fixtures for java2rust-rs.
 *
 * For each (name, javaInput) it writes:
 *   <outDir>/<name>.java          the raw Java input
 *   <outDir>/<name>.rs.expected   JavaConverter.convert2Rust(input)
 *
 * The corpus is taken verbatim from the original JUnit tests, plus a few extras.
 * Usage: java -cp java2rust.jar:tools GenGolden <outDir>
 */
public class GenGolden {

    static Map<String, String> corpus() {
        Map<String, String> c = new LinkedHashMap<>();

        // ---- DeclarationsTest ----
        c.put("decl_field", "class A { int i; }");
        c.put("decl_field_init", "class A { int i = 1; }");
        c.put("decl_method_param", "class A { void m(int i) { }; }");
        c.put("decl_local", "class A { void m() { int i; }; }");
        c.put("decl_local_init", "class A { void m() { int i = 2; }; }");
        c.put("decl_self_param", "void method() { }");
        c.put("decl_static_method", "static void staticMethod() { }");
        c.put("decl_enum_blocks",
                "class X {\n" +
                " enum A { AA; private final int id; }\n" +
                " enum B { BB; private final int id; }\n" +
                "}");

        // ---- ElseTest ----
        c.put("new_class", "new Class()");
        c.put("new_class_arg", "new Class(i)");

        // ---- SnakeTest ----
        c.put("snake_method_call", "xAA.xAAB()");
        c.put("snake_var_decl", "String xTestString;");
        c.put("snake_method_decl", "String methodMIs();");
        c.put("snake_test_anno", "@Test void testMethod() { int i; }");
        c.put("snake_test_anno_nl", "@Test\n void testMethod() { int a; }");
        c.put("snake_test_anno_expected", "@Test(expected = Exception.class)\n void testMethod() { int b; }");

        // ---- ForTester ----
        c.put("for_complete", "for (int i = 10; i < 100; i++) { System.out.println(\"i: \" + i); }");
        c.put("for_no_cond", "for (int i = 10; ; i++) { System.out.println(\"i: \" + i); if (i > 100) break; }");
        c.put("for_empty", "int i = 0; for (;;) { System.out.println(\"i: \" + i); if (i > 100) break; else { i++; } }");
        c.put("for_only_inc", "int i = 0; for (;;i++) { System.out.println(\"i: \" + i); if (i > 100) break;  }");

        // ---- StringExpConvTest ----
        c.put("string_exp_decl", " class A { void m() { String s = \"5 choose \" + i + \"gdgahdgs\"; };  }  ");

        // ---- IntegerConvTest ----
        c.put("int_arrays_1", " class A { void m() { double[] testArray = {0, 1, +2., 3.1, -4, 5 * 10}; };  }  ");
        c.put("int_arrays_2", " class A { void m() { double[][] testArray = {{0, 1, +2., 3.1, -4, 5}}; };  }  ");
        c.put("int_decl", " class A { void m() { double test = 1; float x = 10; };  }  ");
        c.put("int_expr_1", " class A { void m() { double test = 1; test = 10 * 1.5; float x = 10; };  }  ");
        c.put("int_expr_2", " class A { void m() { double test = 1; test = 10 * 1.5; float x = 10; int x2 = 20; };  }  ");
        c.put("int_for_expr", " class A { void m() { for (int i = 0; i < 10.0; i++) { } };  }  ");
        c.put("int_for_expr2", " class A { void m() {" +
                "final int from = 3;\n" +
                "    final int to = source.length + 14;\n" +
                "    final double[] dest = MathArrays.copyOfRange(source, from, to);\n" +
                "\n" +
                "    Assert.assertEquals(dest.length, to - from);\n" +
                "    for (int i = from; i < source.length; i++) {\n" +
                "        Assert.assertEquals(source[i + 1], dest[i - from + 1], 0);\n" +
                "    } };  }  ");
        c.put("int_const", " {\n" +
                "        int j = 0;             \n" +
                "        final double[] u = { 0};       \n" +
                "    }\n  ");
        c.put("int_parameter", " class A { void m(double x) { m(10); };  }  ");

        // ---- StackoverflowTest ----
        c.put("stackoverflow_1", "class A { "
                             + "   public String toString() { "
                             + "     if (!signalDetected()) { "
                             + "       return \"IR Seeker: --% signal at ---.- degrees\"; "
                             + "     }"
                             + "     return String.format(\"IR Seeker: %3.0f%% signal at %6.1f degrees\", getStrength() * 100.0d, getAngle()); "
                             + "   }"
                             + "   public static void throwIfModernRoboticsI2cAddressIsInvalid(int newAddress) { "
                             + "     if ((newAddress < MIN_NEW_I2C_ADDRESS) ||"
                             + "         (newAddress > MAX_NEW_I2C_ADDRESS)) { "
                             + "       throw new IllegalArgumentException(String.format(\"New I2C address %d is invalid; "
                             + "                     valid range is: %d..%d\", newAddress, MIN_NEW_I2C_ADDRESS, MAX_NEW_I2C_ADDRESS)); "
                             + "     } "
                             + "     else if ((newAddress % 2) != 0) "
                             + "     { "
                             + "       throw new IllegalArgumentException(String.format(\"New I2C address %d is invalid; the address must be even.\", newAddress)); "
                             + "     } "
                             + "   } ; "
                             + "}");

        // ---- CommentTest ----
        c.put("comment_interface",
                "/**\n" +
                " * Interface comment\n" +
                " */\n" +
                "public interface X {\n" +
                "  /**\n" +
                "   * Hello\n" +
                "   */\n" +
                "  // World\n" +
                "  int hello();\n" +
                "\n" +
                "  /**\n" +
                "   * Just javadoc\n" +
                "   */\n" +
                "   int ohyes();\n" +
                "}\n" +
                "");
        c.put("comment_package",
                "/**\n" +
                " * Licence\n" +
                " */\n" +
                "// Comment\n" +
                "package y;\n" +
                "\n" +
                "/**\n" +
                " * Class.\n" +
                " */\n" +
                "public class C{}");

        // ---- generalization cases (beyond the original JUnit inputs) ----
        c.put("gen_ternary", "class T { int m(int a){ return a > 0 ? a : -a; } }");
        c.put("gen_strcat", "class S { String g(int n){ return \"n=\" + n + \"!\"; } }");
        c.put("gen_whilecast", "class W { void m(){ int i = 0; while (i < 10) { i++; } double d = (double) i; } }");
        c.put("gen_switch", "class Sw { void m(int x){ switch(x){ case 1: return; default: break; } } }");
        c.put("gen_nested_if", "class N { void m(){ if (a) { if (b) { c(); } else { d(); } } } }");
        c.put("gen_fields", "class F { public static final int K = 5; private String name; boolean flag = true; }");
        c.put("gen_labeled", "class L { void m(){ outer: for(int i=0;i<3;i++){ break outer; } } }");
        c.put("gen_sync", "class Y { void m(Object o){ synchronized(o){ x(); } } }");
        c.put("gen_calc",
                "package com.example;\n" +
                "import java.util.List;\n" +
                "public class Calc {\n" +
                "    private int total = 0;\n" +
                "    public int addAll(int[] xs) {\n" +
                "        for (int i = 0; i < xs.length; i++) {\n" +
                "            total += xs[i];\n" +
                "        }\n" +
                "        if (total > 100) { return 100; } else { return total; }\n" +
                "    }\n" +
                "    static double half(double v) { return v / 2; }\n" +
                "}");

        return c;
    }

    public static void main(String[] args) throws Exception {
        Path outDir = Paths.get(args.length > 0 ? args[0] : "tests/corpus");
        Files.createDirectories(outDir);

        PrintStream realOut = System.out;
        // Silence the converter's internal debug prints while converting.
        PrintStream nullOut = new PrintStream(new OutputStream() {
            public void write(int b) {}
        });

        for (Map.Entry<String, String> e : corpus().entrySet()) {
            String name = e.getKey();
            String input = e.getValue();
            System.setOut(nullOut);
            String output;
            try {
                output = JavaConverter.convert2Rust(input);
            } finally {
                System.setOut(realOut);
            }
            Files.write(outDir.resolve(name + ".java"), input.getBytes(StandardCharsets.UTF_8));
            Files.write(outDir.resolve(name + ".rs.expected"), output.getBytes(StandardCharsets.UTF_8));
            realOut.println("wrote " + name + " (" + output.length() + " bytes)");
        }
    }
}
