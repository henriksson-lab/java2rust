public class JavaTextKernelOracle {
    static String units(String value) {
        StringBuilder text=new StringBuilder();
        for(int i=0;i<value.length();i++) {if(i>0)text.append(',');text.append(Integer.toHexString(value.charAt(i)));}
        return text.toString();
    }
    static void out(String key,Object value) {System.out.println(key+"="+value);}
    public static void main(String[] args) {
        int[][] arrays={{},{65},{0},{65,233,55357,56832,90},{55357},{56832},{55357,56832},
            {56832,55357},{55296,55296,56320},{65535,0,1},{97,98,97,98,97},{55357,65,56832},{10},{233,233,233}};
        String[] strings=new String[arrays.length];
        for(int i=0;i<arrays.length;i++) {char[] chars=new char[arrays[i].length];for(int j=0;j<chars.length;j++)chars[j]=(char)arrays[i][j];strings[i]=new String(chars);}
        int[] offsets={-2,-1,0,1,2,5,Integer.MAX_VALUE};
        int[] points={-1,0,65,233,55357,56832,65535,65536,128512,1114111,1114112};
        String empty="";
        for(int i=0;i<strings.length;i++) {
            String s=strings[i];String k="s."+i;
            out(k+".units",units(s));out(k+".length",s.length());out(k+".hash",s.hashCode());
            String alias=s;String fresh=new String(s);
            out(k+".cloneIdentity",s==alias);out(k+".freshIdentity",s==fresh);
            out(k+".internIdentity",s.intern()==fresh.intern());
            out(k+".wholeSliceIdentity",s.substring(0,s.length())==s);
            out(k+".trimUnits",units(s.trim()));out(k+".trimIdentity",s.trim()==s);
            out(k+".emptyConcatIdentity",s.concat(empty)==s);
            out(k+".emptyAppendIdentity",(s+empty)==s);
            out(k+".emptySliceIdentity",s.substring(s.length(),s.length())==empty);
            for(int n=-1;n<=s.length();n++) {
                try {out(k+".char."+n,Integer.toHexString(s.charAt(n)));}
                catch(StringIndexOutOfBoundsException expected) {out(k+".char."+n,"ERR");}
            }
            for(int b=-1;b<=s.length()+1;b++)for(int e=-1;e<=s.length()+1;e++) {
                try {out(k+".sub."+b+"."+e,units(s.substring(b,e)));}
                catch(StringIndexOutOfBoundsException expected) {out(k+".sub."+b+"."+e,"ERR");}
            }
            StringBuilder builder = new StringBuilder(s);
            builder.append((char)0xd800); out(k+".buffer.append",units(builder.toString()));
            builder.setCharAt(0,(char)0xdc00); out(k+".buffer.set",units(builder.toString()));
            builder.setLength(1); out(k+".buffer.truncate",units(builder.toString()));
            builder.setLength(3); out(k+".buffer.grow",units(builder.toString()));
            builder.insert(1,new char[]{(char)0xd83d,(char)0xde00}); out(k+".buffer.insert",units(builder.toString()));
            builder.delete(1,2); out(k+".buffer.delete",units(builder.toString()));
            builder.replace(0,2,"é"); out(k+".buffer.replace",units(builder.toString()));
            for(int j=0;j<strings.length;j++) {
                String t=strings[j];String p=k+".other."+j;
                out(p+".eq",s.equals(t));out(p+".cmp",s.compareTo(t));out(p+".concat",units(s.concat(t)));
                out(p+".concatSelfIdentity",s.concat(t)==s);out(p+".concatOtherIdentity",s.concat(t)==t);
                for(int offset:offsets) {out(p+".index."+offset,s.indexOf(t,offset));out(p+".last."+offset,s.lastIndexOf(t,offset));}
            }
            for(int point:points)for(int offset:new int[]{-1,0,1,5})out(k+".point."+point+"."+offset,s.indexOf(point,offset));
        }
        String unique=new String(new char[]{(char)0xd832,(char)0x0001,(char)0xffff,(char)0xdc22,(char)0x0327});
        out("uniqueFirstInternIdentity",unique.intern()==unique);
    }
}
