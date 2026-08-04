public class P6 { public int sum(int... xs){ int t=0; for(int x: xs) t+=x; return t; } public int run(){ return sum(1,2,3); } }
