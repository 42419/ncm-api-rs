use super::Query;
use crate::error::Result;
/// 购买数字专辑
/// 对应 Node.js module/digitalAlbum_ordering.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 购买数字专辑
    /// 对应 /digitalAlbum/ordering
    pub async fn digital_album_ordering(&self, query: &Query) -> Result<ApiResponse> {
        // 上游 `paymentMethod: query.payment`、内层 `resourceID: query.id` /
        // `quantity: query.quantity` 均为裸取值，缺省时丢键
        let mut resource = json!({
            "business": "Album",
            "resourceID": query.get_or("id", ""),
            "quantity": query.get_or("quantity", "1")
        });
        query.drop_absent_params(
            &mut resource,
            &[("resourceID", "id"), ("quantity", "quantity")],
        );
        let mut data = json!({
            "business": "Album",
            "paymentMethod": query.get_or("payment", ""),
            "digitalResources": serde_json::to_string(&json!([resource])).unwrap_or_default(),
            "from": "web"
        });
        query.drop_absent_params(&mut data, &[("paymentMethod", "payment")]);
        self.request(
            "/api/ordering/web/digital",
            data,
            query.to_option(CryptoType::Weapi),
        )
        .await
    }
}
