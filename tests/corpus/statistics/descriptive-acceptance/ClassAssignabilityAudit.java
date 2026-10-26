package classassignability;
public class ClassAssignabilityAudit {
    interface Tag {}
    static class Root { int value; }
    static class Mid extends Root implements Tag {}
    static class Leaf extends Mid {}
    static class Sibling extends Root {}
    static Root root(boolean leaf) { return leaf ? new Leaf() : new Sibling(); }
    static boolean assign(Class<?> target,Class<?> actual) { return target.isAssignableFrom(actual); }
    static int effects;
    static Class<?> observed() { effects++; return Leaf.class; }
    public static int run() {
        if(!Root.class.isAssignableFrom(Leaf.class) || Leaf.class.isAssignableFrom(Root.class)) return 1;
        if(!Mid.class.isAssignableFrom(Leaf.class) || Mid.class.isAssignableFrom(Sibling.class)) return 2;
        if(!Tag.class.isAssignableFrom(Leaf.class) || Tag.class.isAssignableFrom(Sibling.class)) return 3;
        Root value=root(true);
        if(!assign(Mid.class,value.getClass()) || assign(Sibling.class,value.getClass())) return 4;
        if(!value.getClass().getName().equals("classassignability.ClassAssignabilityAudit$Leaf")) return 5;
        value=root(false);
        if(assign(Mid.class,value.getClass()) || !assign(Root.class,value.getClass())) return 6;
        if(!Object.class.isAssignableFrom(Tag.class) || Object.class.isAssignableFrom(int.class)) return 7;
        if(!int.class.isAssignableFrom(int.class) || double.class.isAssignableFrom(int.class)) return 8;
        if(!Object.class.isAssignableFrom(int[].class) || Object[].class.isAssignableFrom(int[].class)) return 9;
        if(!Object[].class.isAssignableFrom(int[][].class)) return 10;
        if(!Root[].class.isAssignableFrom(Leaf[].class) || Leaf[].class.isAssignableFrom(Root[].class)) return 11;
        if(!Cloneable.class.isAssignableFrom(int[].class) || !java.io.Serializable.class.isAssignableFrom(Root[][].class)) return 12;
        if(!Leaf[].class.getName().equals("[Lclassassignability.ClassAssignabilityAudit$Leaf;")) return 13;
        Class<?> absent=null;
        try { assign(Root.class,absent); return 14; }
        catch(NullPointerException expected) {}
        effects=0;
        try { absent.isAssignableFrom(observed()); return 15; }
        catch(NullPointerException expected) { if(effects!=1) return 16; }
        if(!assign(Root.class,Leaf.class) || assign(Leaf.class,Sibling.class)) return 17;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
