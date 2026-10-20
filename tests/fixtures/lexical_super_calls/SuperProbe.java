package neutral.supercalls;

class SuperLeaf extends SuperChild<String> { }

public class SuperProbe {
    public static int run() {
        SuperChild<String> child = new SuperChild<>();
        SuperChild<String> alias = child;
        if (!child.echo("first").equals("first") || alias.calls != 1) return 1;
        if (child.inheritedMark("second") != 3 || alias.calls != 3) return 2;
        if (child.self() != child) return 3;
        SuperLeaf leaf = new SuperLeaf();
        if (!leaf.echo("leaf").equals("leaf") || leaf.calls != 1) return 4;
        if (leaf.inheritedMark("leaf") != 3 || leaf.calls != 3) return 5;
        if (leaf.checked(false) != 4) return 6;
        try { leaf.checked(true); return 7; }
        catch (IllegalStateException expected) { }
        if (leaf.calls != 5) return 8;
        if (child.viaSuperCallback(n -> n + 4, 5) != 9) return 9;
        if (leaf.viaSuperCallback(n -> n + 2, 3) != 5) return 10;
        try { leaf.viaSuperCallback(n -> 42 / n, 0); return 11; }
        catch (ArithmeticException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
