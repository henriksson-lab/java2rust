package idiom.gaps;

public class SwitchReturnsEnumMatch {
    enum Type {
        SNP,
        INDEL,
        SYMBOLIC
    }

    public String label(Type type) {
        switch (type) {
            case SNP:
                return "snp";
            case INDEL:
                return "indel";
            case SYMBOLIC:
                return "symbolic";
            default:
                return "unknown";
        }
    }
}
