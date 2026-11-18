import { getSupabaseBrowserClient } from "@/lib/supabase";
const API_URL = process.env.NEXT_PUBLIC_ATLAS_API_URL || "http://localhost:8080/api/v1";
export class ApiError extends Error { constructor(message: string, public status: number) { super(message); this.name="ApiError"; } }
export async function api<T>(path:string, init:RequestInit={}):Promise<T> {
 const {data}=await getSupabaseBrowserClient().auth.getSession();
 if(!data.session?.access_token) throw new ApiError("Your session has expired. Sign in again.",401);
 const headers=new Headers(init.headers); headers.set("Authorization","Bearer "+data.session.access_token);
 if(init.body&&!headers.has("Content-Type")) headers.set("Content-Type","application/json");
 const response=await fetch(API_URL+path,{...init,headers,cache:"no-store"});
 if(response.status===204) return undefined as T;
 const payload=await response.json().catch(()=>({}));
 if(!response.ok) throw new ApiError(payload.message||payload.error||"Request failed.",response.status);
 return payload as T;
}
export const jsonBody=(value:unknown)=>JSON.stringify(value);