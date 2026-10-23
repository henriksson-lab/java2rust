package org.apache.commons.numbers.complex;

import java.nio.file.Files;
import java.nio.file.Path;

/** Caller only: operation inputs and independent expected values are pinned test data. */
public class ComplexReferenceOracle {
    static Complex apply(String operation, Complex a, Complex b) {
        switch (operation) {
        case "acos": return a.acos();
        case "acosh": return a.acosh();
        case "asinh": return a.asinh();
        case "atanh": return a.atanh();
        case "cosh": return a.cosh();
        case "sinh": return a.sinh();
        case "tanh": return a.tanh();
        case "exp": return a.exp();
        case "log": return a.log();
        case "sqrt": return a.sqrt();
        case "multiply": return a.multiply(b);
        case "divide": return a.divide(b);
        case "pow": return a.pow(b);
        default: throw new IllegalArgumentException("unknown reference operation " + operation);
        }
    }
    public static void main(String[] args) throws Exception {
        for (String line : Files.readAllLines(Path.of(args[0]))) {
            String[] fields = line.split("\t");
            if (fields.length != 7 && fields.length != 9) throw new IllegalArgumentException(line);
            Complex a = Complex.ofCartesian(Double.parseDouble(fields[3]), Double.parseDouble(fields[4]));
            Complex b = fields.length == 9 ? Complex.ofCartesian(Double.parseDouble(fields[5]), Double.parseDouble(fields[6])) : null;
            Complex result = apply(fields[0], a, b);
            System.out.println("reference." + fields[0] + "." + fields[1] + "=" +
                Long.toHexString(Double.doubleToRawLongBits(result.real())) + "," +
                Long.toHexString(Double.doubleToRawLongBits(result.imag())));
        }
        System.out.println("oracle=ok");
    }
}
