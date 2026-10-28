import java.io.*;
public class CapabilityCastContracts {
    Appendable target;
    static int effects;
    static String effect() { effects++; return "q"; }
    static Writer view(Appendable value) { return (Writer)value; }
    void writeField() throws IOException { ((Writer)target).write(effect()); }
    static void writeValue(Writer out) throws IOException { out.write("x\ud800"); }
    static void write(Appendable out) throws IOException {
        if (out instanceof Writer) writeValue((Writer)out);
    }
    static void flushAndClose(Appendable out) throws IOException {
        if (out instanceof Flushable) ((Flushable)out).flush();
        if (out instanceof Closeable) ((Closeable)out).close();
    }
    static String trim(CharSequence text) {
        if (text instanceof String) return ((String)text).trim();
        return "other";
    }
    static int run() throws Exception {
        StringWriter text = new StringWriter();
        Writer writer = text;
        Appendable out = writer;
        write(out);
        flushAndClose(out);
        if (!text.toString().equals("x\ud800")) return 1;
        if (!trim(" a\ud800 ").equals("a\ud800")) return 2;
        Writer projected = (Writer)out;
        writeValue(projected);
        if (!text.toString().equals("x\ud800x\ud800")) return 3;
        Writer absent = (Writer)(Appendable)null;
        if (absent != null) return 4;
        try { ((Writer)(Appendable)new StringBuilder()).write("bad"); return 5; }
        catch (ClassCastException expected) { }
        try { ((Closeable)(Appendable)new StringBuilder()).close(); return 6; }
        catch (ClassCastException expected) { }
        try { ((String)(CharSequence)new StringBuilder("bad")).trim(); return 7; }
        catch (ClassCastException expected) { }
        try { ((Closeable)(Appendable)null).close(); return 8; }
        catch (NullPointerException expected) { }
        CapabilityCastContracts holder = new CapabilityCastContracts();
        holder.target = out;
        holder.writeField();
        if (effects != 1 || !text.toString().equals("x\ud800x\ud800q")) return 9;
        holder.target = null;
        if (view(holder.target) != null) return 10;
        try { holder.writeField(); return 11; }
        catch (NullPointerException expected) { }
        if (effects != 2) return 12;
        holder.target = new StringBuilder();
        try { holder.writeField(); return 13; }
        catch (ClassCastException expected) { }
        if (effects != 2) return 14;
        holder.target = out;
        ((Writer)holder.target).write("abc", 1, 1);
        if (!text.toString().equals("x\ud800x\ud800qb")) return 15;
        return 0;
    }
    public static void main(String[] args) throws Exception { System.out.println(run()); }
}
