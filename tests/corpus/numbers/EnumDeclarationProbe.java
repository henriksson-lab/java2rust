enum EnumDeclaration {
    FIRST, SECOND;
    EnumDeclaration link;
    static final EnumDeclaration ALIAS = FIRST;
    static EnumDeclaration selected = SECOND;
}

public class EnumDeclarationProbe {
    static int run() {
        if (EnumDeclaration.values().length != 2) return 1;
        if (EnumDeclaration.FIRST.ordinal() != 0 || EnumDeclaration.SECOND.ordinal() != 1) return 2;
        if (!EnumDeclaration.FIRST.name().equals("FIRST")) return 3;
        if (EnumDeclaration.ALIAS != EnumDeclaration.FIRST) return 4;
        if (EnumDeclaration.selected != EnumDeclaration.SECOND) return 5;
        EnumDeclaration.selected = EnumDeclaration.ALIAS;
        if (EnumDeclaration.selected != EnumDeclaration.FIRST) return 6;
        if (EnumDeclaration.values().length != 2) return 7;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
