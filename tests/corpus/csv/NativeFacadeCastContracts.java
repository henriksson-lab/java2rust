import java.io.Reader;
import java.sql.Clob;
import java.sql.SQLException;

public class NativeFacadeCastContracts {
    static int effects;
    static Reader guarded(Object value) throws SQLException {
        return value instanceof Clob ? ((Clob)value).getCharacterStream() : null;
    }
    static Clob cast(Object value) { return (Clob)value; }
    static Reader direct(Object value) throws SQLException { return ((Clob)value).getCharacterStream(); }
    static Reader changed(Object value) throws SQLException {
        if (value != null) value = null;
        return ((Clob)value).getCharacterStream();
    }
    static Object source(boolean wrong) throws SQLException {
        effects = effects * 10 + 1;
        if (guarded("reentry") != null) throw new IllegalStateException();
        return wrong ? "wrong" : null;
    }
    static int after() { effects = effects * 10 + 2; return 7; }
    static void consume(Clob value, int marker) {
        effects = effects * 10 + 3;
        if (value != null || marker != 7) throw new IllegalStateException();
    }
    static int run() throws SQLException {
        if (guarded("text") != null || guarded(null) != null || cast(null) != null) return 1;
        try { cast("wrong"); return 2; } catch (ClassCastException expected) {}
        try { direct("wrong"); return 3; } catch (ClassCastException expected) {}
        try { direct(null); return 4; } catch (NullPointerException expected) {}
        effects = 0;
        consume((Clob)source(false), after());
        if (effects != 123) return 5;
        effects = 0;
        try { consume((Clob)source(true), after()); return 6; } catch (ClassCastException expected) {}
        if (effects != 1) return 7;
        try { changed("before"); return 8; } catch (NullPointerException expected) {}
        return 0;
    }
    public static void main(String[] args) throws SQLException { System.out.println(run()); }
}
