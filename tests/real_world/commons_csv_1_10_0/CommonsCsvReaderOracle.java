package org.apache.commons.csv;

import java.io.IOException;
import java.io.StringReader;

/** Package access exercises the untouched production reader implementation. */
public final class CommonsCsvReaderOracle {
    public static String closedLookAhead() throws IOException {
        ExtendedBufferedReader reader = new ExtendedBufferedReader(new StringReader("a"));
        reader.close();
        String array;
        try { reader.lookAhead(new char[1]); array = "accepted"; }
        catch (IOException expected) { array = "IOException"; }
        String scalar;
        try { reader.lookAhead(); scalar = "accepted"; }
        catch (IOException expected) { scalar = "IOException"; }
        return array + "|" + scalar;
    }
}
