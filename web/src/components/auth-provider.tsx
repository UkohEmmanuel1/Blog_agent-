"use client";
import {createContext,useContext,useEffect,useState} from "react";
import type {Session} from "@supabase/supabase-js";
import {getSupabaseBrowserClient} from "@/lib/supabase";
type AuthValue={session:Session|null;loading:boolean;signOut:()=>Promise<void>};
const AuthContext=createContext<AuthValue|null>(null);
export function AuthProvider({children}:{children:React.ReactNode}) {
 const [session,setSession]=useState<Session|null>(null); const [loading,setLoading]=useState(true);
 useEffect(()=>{const supabase=getSupabaseBrowserClient();supabase.auth.getSession().then(({data})=>{setSession(data.session);setLoading(false)});const {data}=supabase.auth.onAuthStateChange((_event,next)=>{setSession(next);setLoading(false)});return()=>data.subscription.unsubscribe()},[]);
 async function signOut(){await getSupabaseBrowserClient().auth.signOut();setSession(null)}
 return <AuthContext.Provider value={{session,loading,signOut}}>{children}</AuthContext.Provider>
}
export function useAuth(){const value=useContext(AuthContext);if(!value)throw new Error("useAuth must be used inside AuthProvider");return value}