package accessorboundaries;
public class AccessProbe {
    static int effects;
    static Cell factory(Cell value) { effects++; return value; }
    static int width() { return Integer.BYTES * 3; }
    static long overflow() { return Integer.MAX_VALUE * 2; }
    static long wide() { return (long) Integer.MAX_VALUE * 2; }
    public static int run() {
        Cell cell=new Cell(7); Holder holder=new Holder(cell);
        Cell alias=holder.payload(); alias.write(11);
        if(cell.read()!=11 || holder.payload()!=cell) return 1;
        effects=0;
        if(factory(cell).read()!=11 || effects!=1) return 2;
        factory(cell).write(17);
        if(cell.read()!=17 || effects!=2) return 3;
        Cell missing=null;
        try { missing.read(); return 4; } catch(NullPointerException expected) {}
        try { missing.write(19); return 5; } catch(NullPointerException expected) {}
        if(width()!=12 || overflow()!=-2L || wide()!=4294967294L) return 6;
        java.nio.ByteBuffer buffer=java.nio.ByteBuffer.allocate(Long.BYTES * 2);
        buffer.putInt(13);
        if(buffer.capacity()!=16 || buffer.position()!=4 || buffer.remaining()!=12) return 7;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
