<template>
    <b-alert :variant="variant" fade :show="alertVisible" @dismissed="dismiss" class="mt-2" v-html="message">
    </b-alert>
</template>

<script>
export default {
    name: 'Alert',
    data: () => ({
        alertVisible: false,
        message: '',
        variant: 'info',
        timeoutHandle: null,
    }),
    methods: {
        dismiss() {
            window.clearTimeout(this.timeoutHandle);
            this.alertVisible = false;
        },
        display(options) {
            let message = options.message || 'default message';
            let variant = options.variant || this.variant;
            let countdown = options.countdown || 0;

            this.message = message;
            this.variant = variant;
            this.alertVisible = true;

            window.clearTimeout(this.timeoutHandle);
            
            if (countdown) {
                const self = this;
                this.timeoutHandle = setTimeout(() => {
                    self.dismiss();
                }, countdown * 1000);
            }
        }
    }
}
</script>
