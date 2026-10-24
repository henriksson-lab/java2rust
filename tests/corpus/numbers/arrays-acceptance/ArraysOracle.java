package org.apache.commons.numbers.arrays;

import java.util.Arrays;

/** Independent calls into the untouched production closure; never translated. */
public final class ArraysOracle {
    interface Action { void run(); }
    static String status(Action action) {
        try { action.run(); return "OK"; }
        catch (RuntimeException error) {
            String message = error.getMessage();
            StringBuilder text = new StringBuilder(error.getClass().getName()).append(':');
            if (message == null) text.append("NULL");
            else for (int i = 0; i < message.length(); i++) text.append(String.format("%04x", (int)message.charAt(i)));
            return text.toString();
        }
    }
    static String view(int[] values) { return Arrays.toString(values); }
    static String view(long[] values) { return Arrays.toString(values); }
    static String view(double[] values) {
        StringBuilder text = new StringBuilder("[");
        for (int i = 0; i < values.length; i++) {
            if (i != 0) text.append(", ");
            text.append(Long.toHexString(Double.doubleToRawLongBits(values[i])));
        }
        return text.append(']').toString();
    }
    static int[] data(int n, int pattern) {
        int[] values = new int[n]; int state = 0x12345678 + pattern * 0x123;
        for (int i = 0; i < n; i++) {
            state = state * 1664525 + 1013904223;
            values[i] = pattern == 0 ? n-i : pattern == 1 ? i*17%11-5 :
                        pattern == 2 ? state : pattern == 3 ? i : (i%2 == 0 ? i : -i);
        }
        return values;
    }
    static long[] longs(int[] input) {
        long[] values = new long[input.length];
        for (int i = 0; i < values.length; i++) values[i] = ((long)input[i] << 32) ^ (0x9e3779b9L+i);
        return values;
    }
    static double[] doubles(int[] input, int pattern) {
        double[] values = new double[input.length];
        for (int i = 0; i < values.length; i++) {
            values[i] = input[i] / 8.0;
            if (pattern == 4) {
                if (i%13 == 0) values[i] = -0.0;
                else if (i%29 == 0) values[i] = 0.0;
                else if (i%17 == 0) values[i] = Double.longBitsToDouble(0x7ff8000000000042L+i%3);
                else if (i%19 == 0) values[i] = i%2 == 0 ? Double.POSITIVE_INFINITY : Double.NEGATIVE_INFINITY;
            }
        }
        return values;
    }
    static void selection() {
        int[] lengths = {0,1,2,3,5,16,31,64,129,512,2048,5000};
        for (int type = 0; type < 3; type++) for (int n : lengths) for (int pattern = 0; pattern < 5; pattern++) {
            for (int variant = 0; variant < 4; variant++) for (int query = 0; query < 6; query++) {
                int[] base = data(n,pattern);
                int from = variant >= 2 && n >= 2 ? 1 : 0;
                int to = variant >= 2 && n >= 2 ? n-1 : n;
                int k = query == 0 ? from : query == 1 ? from+(to-from)/2 : query == 2 ? to-1 : query == 4 ? from+(to-from)/4 : query == 5 ? from+3*(to-from)/4 : to;
                int[] indices = query == 0 ? new int[0] : query == 2 ? new int[]{k,from,to-1,k} : new int[]{k};
                if (query == 4 && to > from) {
                    indices = new int[17];
                    for (int i = 0; i < indices.length; i++) indices[i] = from+(to-from-1)*(16-i)/16;
                }
                final int[] requested = indices;
                final int v = variant;
                String result; String values;
                if (type == 0) {
                    double[] a = doubles(base,pattern);
                    result = status(() -> { switch (v) {
                        case 0: Selection.select(a,k); break;
                        case 1: Selection.select(a,requested); break;
                        case 2: Selection.select(a,from,to,k); break;
                        default: Selection.select(a,from,to,requested);
                    }}); values = view(a);
                } else if (type == 1) {
                    int[] a = base;
                    result = status(() -> { switch (v) {
                        case 0: Selection.select(a,k); break;
                        case 1: Selection.select(a,requested); break;
                        case 2: Selection.select(a,from,to,k); break;
                        default: Selection.select(a,from,to,requested);
                    }}); values = view(a);
                } else {
                    long[] a = longs(base);
                    result = status(() -> { switch (v) {
                        case 0: Selection.select(a,k); break;
                        case 1: Selection.select(a,requested); break;
                        case 2: Selection.select(a,from,to,k); break;
                        default: Selection.select(a,from,to,requested);
                    }}); values = view(a);
                }
                System.out.println("selection:"+type+":"+n+":"+pattern+":"+variant+":"+query+"\t"+result+"\t"+values+"\t"+(variant%2 == 1 ? view(indices) : "-"));
            }
        }
        int[][] aliases = {{3,0,1,2},{3,0,0,2},{2,2,0,3},{1,1,1,1}};
        for (int i = 0; i < aliases.length; i++) for (int v = 0; v < 2; v++) {
            int[] a = aliases[i].clone(); final int variant = v;
            String result = status(() -> { if (variant == 0) Selection.select(a,a); else Selection.select(a,0,4,a); });
            System.out.println("selection-alias:"+i+":"+v+"\t"+result+"\t"+view(a));
        }
    }
    static void ranges() {
        int[][] cases = {{-1,4,1},{0,6,2},{4,2,3},{0,0,0},{5,5,5},{2,4,1}};
        for (int type = 0; type < 3; type++) for (int id = 0; id < cases.length; id++) for (int variant = 0; variant < 2; variant++) {
            final int v = variant;
            int from=cases[id][0], to=cases[id][1], k=cases[id][2];
            int[] indices={k,k}; String result, values;
            if (type == 0) {
                    double[] a = doubles(data(5,0),0);
                    result = status(() -> { if (v == 0) Selection.select(a,from,to,k); else Selection.select(a,from,to,indices); });
                    values = view(a);
                } else if (type == 1) {
                    int[] a = data(5,0);
                    result = status(() -> { if (v == 0) Selection.select(a,from,to,k); else Selection.select(a,from,to,indices); });
                    values = view(a);
                } else {
                    long[] a = longs(data(5,0));
                    result = status(() -> { if (v == 0) Selection.select(a,from,to,k); else Selection.select(a,from,to,indices); });
                    values = view(a);
                }
            System.out.println("selection-range:"+type+":"+id+":"+variant+"\t"+result+"\t"+values+"\t"+view(indices));
        }
    }
    static void sort() {
        String[] invalid = {"", "ascending", "ASCENDING ", "UNKNOWN"};
        for (int i=0; i<invalid.length; i++) {
            final String name = invalid[i];
            System.out.println("enum-invalid:"+i+"\t"+status(() -> SortInPlace.valueOf(name)));
        }
        System.out.println("enum-null\t"+status(() -> SortInPlace.valueOf(null)));
        for (SortInPlace order : SortInPlace.values()) {
            System.out.println("enum:"+order.ordinal()+"\t"+order.name()+"\t"+order+"\t"+(SortInPlace.valueOf(order.name()) == order)+"\t"+order.compareTo(SortInPlace.ASCENDING)+"\t"+order.compareTo(SortInPlace.DESCENDING));
            for (int n : new int[]{0,1,3,8,33}) for (int pattern = 0; pattern < 5; pattern++) for (int alias = 0; alias < 8; alias++) {
                double[] x = doubles(data(n,pattern),pattern); double[] y = new double[n];
                for (int i = 0; i < n; i++) y[i] = 10000+i;
                double[][] rows;
                switch (alias) {
                    case 0: rows = new double[0][]; break;
                    case 1: rows = new double[][]{y}; break;
                    case 2: rows = new double[][]{y,y}; break;
                    case 3: rows = new double[][]{x}; break;
                    case 4: rows = new double[][]{x,x}; break;
                    case 5: rows = new double[][]{x,y,x}; break;
                    case 6: rows = new double[][]{y,new double[n+1]}; break;
                    default: rows = new double[][]{y,null};
                }
                String result = status(() -> order.apply(x,rows));
                System.out.println("sort:"+order.ordinal()+":"+n+":"+pattern+":"+alias+"\t"+result+"\t"+view(x)+"\t"+view(y));
            }
        }
    }
    static void counters() {
        int[][] dimensions = {{1},{2,3},{3,1,2},{1,1,1,1},{2,1,2,1,1,2},{},{0,2},{-1},{2,-3},{65536,65536},{Integer.MAX_VALUE,2}};
        for (int c = 0; c < dimensions.length; c++) {
            final int id = c; int[] sizes = dimensions[c].clone();
            String made = status(() -> {
                MultidimensionalCounter counter = MultidimensionalCounter.of(sizes);
                System.out.println("counter:"+id+":view\t"+counter.getDimension()+"\t"+counter.getSize()+"\t"+view(counter.getSizes())+"\t"+counter);
                sizes[0] = 99;
                int[] copy = counter.getSizes(); copy[0] = 77;
                System.out.println("counter:"+id+":copies\t"+view(counter.getSizes()));
                for (int index = -1; index <= counter.getSize(); index++) {
                    final int i = index;
                    String result = status(() -> {
                        int[] multi = counter.toMulti(i);
                        System.out.println("counter:"+id+":index:"+i+"\t"+view(multi)+"\t"+counter.toUni(multi));
                    });
                    System.out.println("counter:"+id+":index-status:"+i+"\t"+result);
                }
                int[] actual = counter.getSizes();
                for (int slot = 0; slot < actual.length; slot++) for (int upper = 0; upper < 2; upper++) {
                    int[] coordinate = new int[actual.length]; coordinate[slot] = upper == 0 ? -1 : actual[slot];
                    System.out.println("counter:"+id+":invalid:"+slot+":"+upper+"\t"+status(() -> counter.toUni(coordinate)));
                }
                System.out.println("counter:"+id+":arity\t"+status(() -> counter.toUni(new int[actual.length+1])));
            });
            System.out.println("counter:"+id+":construct\t"+made);
        }
    }
    static String interval(UpdatingInterval value, int n) {
        return value.left()+":"+value.right()+":"+IndexSupport.countIndices(value,n);
    }
    static void helpers() {
        for (int capacity : new int[]{-1,0,1,2,7,16}) {
            HashIndexSet set = HashIndexSet.create(capacity);
            int step = 0;
            for (int index : new int[]{0,7,14,0,Integer.MAX_VALUE,21,28,35,42,49,56,63,70,77,84,91,98}) {
                final boolean[] added = {false};
                String result = status(() -> added[0] = set.add(index));
                System.out.println("hash:"+capacity+":"+(step++)+":"+index+"\t"+result+"\t"+added[0]);
                if (!result.equals("OK")) break;
            }
            System.out.println("hash:"+capacity+":negative\t"+status(() -> set.add(-1)));
        }
        System.out.println("hash:capacity\t"+status(() -> HashIndexSet.create((1<<29)+1)));
        int[][] inputs = { {5,3,1,3,5,7}, {1}, {7,7,7}, data(64,3), data(64,0), data(256,3) };
        for (int i = 0; i < inputs.length; i++) {
            int[] keys = inputs[i].clone(); UpdatingInterval value = IndexSupport.createUpdatingInterval(keys,keys.length);
            System.out.println("factory:"+i+"\t"+interval(value,keys.length)+"\t"+view(keys));
        }
        for (int bit = 0; bit < 2; bit++) {
            int[] keys = {1,3,5,7,64,128,130,511};
            UpdatingInterval value;
            if (bit == 0) value = new KeyUpdatingInterval(keys,keys.length);
            else { BitIndexUpdatingInterval bits = new BitIndexUpdatingInterval(1,511); for (int k : keys) bits.set(k); value = bits; }
            System.out.println("interval:"+bit+":initial\t"+interval(value,keys.length));
            value.updateLeft(3); value.updateRight(130);
            UpdatingInterval lower = value.splitLeft(7,64);
            System.out.println("interval:"+bit+":split\t"+interval(lower,keys.length)+"\t"+interval(value,keys.length));
            lower.updateLeft(5); value.updateRight(128);
            System.out.println("interval:"+bit+":reuse\t"+interval(lower,keys.length)+"\t"+interval(value,keys.length));
        }
        for (int n : new int[]{1,3,20}) for (int operation = 0; operation < 2; operation++) {
            int[] keys = data(n,1); for (int i = 0; i < n; i++) keys[i] += 5;
            int count = operation == 0 ? Sorting.insertionSortIndices(keys,n) : Sorting.sortIndices(keys,n);
            System.out.println("indices:"+n+":"+operation+"\t"+count+"\t"+view(keys));
        }
        // Cross SCAN_SIZE so the key interval uses binary rather than linear search.
        int[] many = new int[1024];
        for (int i = 0; i < many.length; i++) many[i] = i * 3 + 1;
        UpdatingInterval large = new KeyUpdatingInterval(many,many.length);
        UpdatingInterval prefix = large.splitLeft(1000,1500);
        System.out.println("interval-large:split\t"+interval(prefix,1024)+"\t"+interval(large,1024));
        prefix.updateLeft(301); large.updateRight(2701);
        System.out.println("interval-large:reuse\t"+interval(prefix,1024)+"\t"+interval(large,1024));
        for (int type = 0; type < 3; type++) for (int pattern = 0; pattern < 5; pattern++) for (int operation = 0; operation < 5; operation++) {
            // These internal helpers require finite values; exceptional doubles
            // are exercised through the public Selection/SortInPlace contracts.
            int[] base = data(9,pattern); String values;
            if (type == 0) {
                double[] a = doubles(base,0);
                switch (operation) {
                    case 0: Sorting.sort(a,1,7); break;
                    case 1: Sorting.sort3(a,1,4,7); break;
                    case 2: Sorting.sort5(a,0,2,4,6,8); break;
                    case 3: Sorting.lowerMedian4(a,1,3,5,7); break;
                    default: Sorting.upperMedian4(a,1,3,5,7);
                } values = view(a);
            } else if (type == 1) {
                int[] a = base;
                switch (operation) {
                    case 0: Sorting.sort(a,1,7); break;
                    case 1: Sorting.sort3(a,1,4,7); break;
                    case 2: Sorting.sort5(a,0,2,4,6,8); break;
                    case 3: Sorting.lowerMedian4(a,1,3,5,7); break;
                    default: Sorting.upperMedian4(a,1,3,5,7);
                } values = view(a);
            } else {
                long[] a = longs(base);
                switch (operation) {
                    case 0: Sorting.sort(a,1,7); break;
                    case 1: Sorting.sort3(a,1,4,7); break;
                    case 2: Sorting.sort5(a,0,2,4,6,8); break;
                    case 3: Sorting.lowerMedian4(a,1,3,5,7); break;
                    default: Sorting.upperMedian4(a,1,3,5,7);
                } values = view(a);
            }
            System.out.println("sorting-helper:"+type+":"+pattern+":"+operation+"\t"+values);
        }
    }
    public static void main(String[] args) {
        switch (args[0]) {
            case "selection": selection(); ranges(); break;
            case "sort": sort(); break;
            case "counter": counters(); break;
            case "helpers": helpers(); break;
            default: throw new IllegalArgumentException("unknown oracle mode");
        }
    }
}
