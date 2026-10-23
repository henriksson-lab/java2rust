package enumlookup;

enum TopChoice { lower_case, UPPER }

public class EnumLookupProbe {
    enum Choice {
        lower_case, UPPER;
        static Choice valueOf(int ordinal) { return ordinal == 0 ? lower_case : UPPER; }
    }
    static class Holder {}

    static Choice lookup(String name) { return Choice.valueOf(name); }

    static int run() {
        if (!Choice.lower_case.name().equals("lower_case")) return 1;
        if (Choice.valueOf("lower_case") != Choice.lower_case) return 2;
        String label = "UPPER";
        if (Choice.valueOf(label) != Choice.UPPER || !label.equals("UPPER")) return 3;
        try { Choice.valueOf("missing"); return 4; }
        catch (IllegalArgumentException error) {
            if (!error.getMessage().equals("No enum constant enumlookup.EnumLookupProbe.Choice.missing")) return 5;
        }
        try { TopChoice.valueOf("missing"); return 6; }
        catch (IllegalArgumentException error) {
            if (!error.getMessage().equals("No enum constant enumlookup.TopChoice.missing")) return 7;
        }
        try { Choice.valueOf((String)null); return 8; }
        catch (NullPointerException error) { if (!error.getMessage().equals("Name is null")) return 9; }
        String absent = null;
        try { Choice.valueOf(absent); return 10; }
        catch (NullPointerException error) { if (!error.getMessage().equals("Name is null")) return 11; }
        if (absent != null) return 12;
        if (!new Holder().getClass().getName().equals("enumlookup.EnumLookupProbe$Holder")) return 13;
        try { lookup("missing"); return 14; }
        catch (IllegalArgumentException error) {
            if (!error.getMessage().equals("No enum constant enumlookup.EnumLookupProbe.Choice.missing")) return 15;
        }
        try { lookup(null); return 16; }
        catch (NullPointerException error) { if (!error.getMessage().equals("Name is null")) return 17; }
        if (Choice.valueOf(0) != Choice.lower_case || Choice.valueOf(1) != Choice.UPPER) return 18;
        try { lookup("missing\ud800"); return 19; }
        catch (IllegalArgumentException error) {
            if (!error.getMessage().equals("No enum constant enumlookup.EnumLookupProbe.Choice.missing\ud800")) return 20;
        }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
