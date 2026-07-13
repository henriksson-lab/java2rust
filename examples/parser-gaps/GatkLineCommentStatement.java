package parser.gaps;

public class GatkLineCommentStatement {
    public String transform(String key, String value) {
        switch (key) {
            case "other":
                // GATK-style comment inside a switch case statement list.
                return value.trim();
            default:
                return value;
        }
    }
}
