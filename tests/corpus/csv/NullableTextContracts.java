class NullableTextNullObject {
    public String toString() { return null; }
}
class NullableTextThrowObject {
    public String toString() { throw new IllegalStateException("text"); }
}

public class NullableTextContracts {
    String saved;
    static CharSequence cast(Object value) {
        CharSequence result;
        if (value == null) {
            result = null;
        } else {
            if (value instanceof CharSequence) result = (CharSequence)value;
            else result = null;
        }
        return result;
    }
    static CharSequence checked(Object value) {
        if (value == null) return (CharSequence)value;
        CharSequence result = (CharSequence)value;
        value.toString();
        return result;
    }
    void save(StringBuilder value) { saved = value.toString(); }
    static int run() {
        CharSequence text = "word";
        CharSequence result = cast(text);
        if (result == null || !result.toString().equals("word")) return 1;
        if (cast(null) != null || checked(null) != null) return 2;
        if (!checked(text).toString().equals("word")) return 4;
        try { checked(Integer.valueOf(17)); return 5; }
        catch (ClassCastException expected) { }
        NullableTextContracts holder = new NullableTextContracts();
        NullableTextContracts alias = holder;
        StringBuilder builder = new StringBuilder();
        builder.append("saved");
        holder.save(builder);
        if (!alias.saved.equals("saved")) return 3;
        CharSequence live = builder;
        CharSequence copied = checked(live);
        builder.append("-late");
        if (!copied.toString().equals("saved-late")) return 6;
        Object nullText = new NullableTextNullObject();
        String nullableResult = nullText.toString();
        if (nullableResult != null) return 7;
        Object throwingText = new NullableTextThrowObject();
        try { throwingText.toString(); return 8; }
        catch (IllegalStateException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
