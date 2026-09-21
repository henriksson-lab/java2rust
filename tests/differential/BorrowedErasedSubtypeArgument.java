public class BorrowedErasedSubtypeArgument {
    static String describe(CompoundLine line) {
        return line.label();
    }

    public static String run() {
        Object erased = new FormatLine();
        FormatLine format = (FormatLine) erased;
        return describe(format);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class HeaderLine {
    String label() {
        return "header";
    }
}

class CompoundLine extends HeaderLine {
    String label() {
        return "compound";
    }
}

class FormatLine extends CompoundLine {
    String label() {
        return "format";
    }
}
