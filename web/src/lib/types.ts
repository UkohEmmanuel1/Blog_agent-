export interface UserProfile {id:string;email:string|null;display_name:string|null;role:"viewer"|"editor"|"reviewer"|"admin"}
export interface Article {id:string;title:string;slug:string;dek:string|null;body_markdown:string;status:string;updated_at:string;published_at:string|null}
export interface Story {id:string;title:string;summary:string|null;canonical_url:string|null;status:string;discovered_at:string}
export interface AgentRun {id:string;task_type:string;status:string;input:Record<string,unknown>;output:unknown;error_message:string|null;created_at:string}
export interface SocialAccount {id:string;platform:string;account_name:string;platform_account_id:string|null;is_active:boolean;created_at:string}
export interface SocialPost {id:string;social_account_id:string;media_asset_id:string|null;content:string;status:string;scheduled_at:string|null;published_at:string|null;platform_url:string|null;created_at:string}
export interface Approval {id:string;resource_type:string;resource_id:string;requested_by:string|null;reviewer_id:string|null;status:string;review_notes:string|null;created_at:string}